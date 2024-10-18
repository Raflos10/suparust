use crate::{Result, Supabase, SupabaseError};
use std::sync::Arc;
pub use supabase_auth::models::{LogoutScope, Session, User};
use tokio::sync::RwLock;

pub const SESSION_REFRESH_GRACE_PERIOD_SECONDS: i64 = 60;

pub struct UpdateUserBuilder {
    user_info: supabase_auth::models::UpdateUserPayload,
    auth: Arc<supabase_auth::models::AuthClient>,
    session: Arc<RwLock<Option<Session>>>,
}

/// A listener for changes to a session
#[derive(Clone)]
pub enum SessionChangeListener {
    Ignore,
    Sync(std::sync::mpsc::Sender<Session>),
    Async(tokio::sync::mpsc::Sender<Session>),
}

impl Supabase {
    async fn set_auth_state(&self, session: Session) {
        *self.session.write().await = Some(session.clone());
        let mut postgrest = self.postgrest.write().await;
        let authorized_postgrest = postgrest
            .clone()
            .insert_header("Authorization", format!("Bearer {}", session.access_token));
        *postgrest = authorized_postgrest;

        match &self.session_listener {
            SessionChangeListener::Ignore => {}
            SessionChangeListener::Sync(sender) => {
                if sender.send(session).is_err() {
                    log::warn!("Failed to send session to listener");
                }
            }
            SessionChangeListener::Async(sender) => {
                if sender.send(session).await.is_err() {
                    log::warn!("Failed to send session to listener");
                }
            }
        }
    }

    /// This function can be used to tell if we most likely have session credentials that are valid.
    /// One use case is to tell if we are logged in or not.
    pub async fn has_valid_auth_state(&self) -> bool {
        self.session.read().await.is_some()
    }

    /// Login with email and password. If successful, the Supabase object will now use the credentials
    /// automatically for all requests. We will also return the session information on success, so that
    /// the caller can e.g. save it for later use (e.g. in calls to `new`).
    pub async fn login_with_email(&self, email: &str, password: &str) -> Result<Session> {
        let session = self.auth.login_with_email(email, password).await?;

        self.set_auth_state(session.clone()).await;

        Ok(session)
    }

    pub(crate) async fn refresh_login(&self) -> crate::Result<()> {
        let auth_state = self.session.read().await.clone();

        if let Some(auth_state) = auth_state {
            let now_epoch = now_as_epoch()?;

            // Refresh some time before the session expires
            let expired =
                (auth_state.expires_at as i64) < now_epoch + SESSION_REFRESH_GRACE_PERIOD_SECONDS;

            if expired {
                match self.auth.refresh_session(auth_state.refresh_token).await {
                    Ok(session) => {
                        self.set_auth_state(session).await;
                    }
                    Err(error) => {
                        if let supabase_auth::error::Error::AuthError { status, .. } = &error {
                            if *status == reqwest::StatusCode::BAD_REQUEST {
                                self.session.write().await.take();
                                return Err(SupabaseError::SessionRefresh(error));
                            }
                        }
                        return Err(SupabaseError::SessionRefresh(error));
                    }
                }
            }
            Ok(())
        } else {
            Err(SupabaseError::MissingAuthenticationInformation)
        }
    }

    /// Log out of the current session. This will invalidate the current session in the Supabase server
    /// and remove it from this Supabase object. Further uses of this object will then not be
    /// authenticated.
    pub async fn logout(&self, scope: Option<LogoutScope>) -> Result<()> {
        self.refresh_login().await?;

        let token = self
            .session
            .read()
            .await
            .as_ref()
            .map(|session| session.access_token.clone())
            .ok_or(SupabaseError::MissingAuthenticationInformation)?;

        self.auth.logout(scope, token).await?;

        self.session.write().await.take();

        Ok(())
    }

    /// If logged in, will return the current user information.
    pub async fn user(&self) -> Option<User> {
        self.session
            .read()
            .await
            .as_ref()
            .map(|session| session.user.clone())
    }

    /// Update the current user. This will return a builder object that can be used to set the different
    /// fields applicable.
    pub async fn update_user(&self) -> Result<UpdateUserBuilder> {
        self.refresh_login().await?;

        Ok(UpdateUserBuilder {
            user_info: supabase_auth::models::UpdateUserPayload {
                email: None,
                password: None,
                data: None,
            },
            auth: self.auth.clone(),
            session: self.session.clone(),
        })
    }
}

impl UpdateUserBuilder {
    /// Send the update request to the server. This will return the updated user information.
    pub async fn send(self) -> Result<User> {
        let token = self
            .session
            .read()
            .await
            .as_ref()
            .map(|session| session.access_token.clone())
            .ok_or(SupabaseError::MissingAuthenticationInformation)?;
        let user = self.auth.update_user(self.user_info, token).await?;

        Ok(user)
    }

    /// Set the email that you want to set your currently logged-in user to have. Remember that the
    /// email is not set until you call `send`.
    pub fn email<StringType: ToString>(mut self, email: StringType) -> Self {
        self.user_info.email = Some(email.to_string());
        self
    }

    /// Set the password that you want to set your currently logged-in user to have. Remember that
    /// the password is not set until you call `send`.
    pub fn password<StringType: ToString>(mut self, password: StringType) -> Self {
        self.user_info.password = Some(password.to_string());
        self
    }
}

#[cfg(target_family = "wasm")]
fn now_as_epoch() -> std::result::Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    Ok(web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)?
        .as_secs() as i64)
}
#[cfg(not(target_family = "wasm"))]
fn now_as_epoch() -> std::result::Result<i64, SupabaseError> {
    Ok(chrono::Utc::now().timestamp())
}
