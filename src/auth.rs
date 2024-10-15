use crate::{Result, SessionChangeListener, Supabase, SupabaseError};
use std::sync::Arc;
use supabase_auth::models::{LogoutScope, Session, User};
use tokio::sync::RwLock;

pub struct UpdateUserBuilder {
    user_info: supabase_auth::models::UpdateUserPayload,
    auth: Arc<supabase_auth::models::AuthClient>,
    session: Arc<RwLock<Option<Session>>>,
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

    pub async fn has_valid_auth_state(&self) -> bool {
        self.session.read().await.is_some()
    }

    pub async fn login_with_email(&self, email: &str, password: &str) -> crate::Result<Session> {
        let session = self.auth.login_with_email(email, password).await?;

        self.set_auth_state(session.clone()).await;

        Ok(session)
    }

    pub(crate) async fn refresh_login(&self) -> crate::Result<()> {
        let auth_state = self.session.read().await.clone();

        if let Some(auth_state) = auth_state {
            let now_epoch = now_as_epoch()?;

            // Refresh 1 minute before expiration
            let expired = (auth_state.expires_at as i64) < now_epoch + 60;

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

    pub async fn logout(&self, scope: Option<LogoutScope>) -> crate::Result<()> {
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

    pub async fn user(&self) -> Option<User> {
        self.session
            .read()
            .await
            .as_ref()
            .map(|session| session.user.clone())
    }

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

    pub fn email<StringType: ToString>(mut self, email: StringType) -> Self {
        self.user_info.email = Some(email.to_string());
        self
    }

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
