mod postgrest;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

extern crate postgrest as external_postgrest;

pub type Result<Type> = std::result::Result<Type, SupabaseError>;

#[derive(Clone)]
pub struct Supabase {
    client: reqwest::Client,
    url: String,
    api_key: String,
    session: Arc<Mutex<Option<Session>>>,
    session_listener: SessionChangeListener,
    postgrest: Arc<RwLock<external_postgrest::Postgrest>>,
}

#[derive(thiserror::Error, Debug)]
pub enum SupabaseError {
    #[error("Failed to refresh session: {0}")]
    SessionRefresh(reqwest::Error),
    #[error("Missing authentication information")]
    MissingAuthenticationInformation,
    #[error("Request failed")]
    Reqwest(#[from] reqwest::Error),
    #[error("Internal error: {0}")]
    Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct Session {
    pub access_token: String,
    pub expires_at: i64,
    pub refresh_token: String,
}

#[derive(Clone)]
pub enum SessionChangeListener {
    Ignore,
    Sync(std::sync::mpsc::Sender<Session>),
    Async(tokio::sync::mpsc::Sender<Session>),
}

impl Supabase {
    pub fn new(
        url: &str,
        api_key: &str,
        session: Option<Session>,
        session_listener: SessionChangeListener,
    ) -> Self {
        let mut postgrest = external_postgrest::Postgrest::new(format!("{url}/rest/v1"))
            .insert_header("apikey", api_key);

        if let Some(session) = &session {
            postgrest = postgrest
                .insert_header("Authorization", format!("Bearer {}", session.access_token));
        }

        Self {
            client: reqwest::Client::new(),
            url: url.to_string(),
            api_key: api_key.to_string(),
            session: Arc::new(Mutex::new(session)),
            session_listener,
            postgrest: Arc::new(RwLock::new(postgrest)),
        }
    }

    async fn set_auth_state(&self, session: Session) {
        *self.session.lock().await = Some(session.clone());
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
        self.session.lock().await.is_some()
    }

    pub async fn authorize(&self, email: &str, password: &str) -> Result<Session> {
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });

        let response = self
            .client
            .post(format!("{}/auth/v1/token", self.url))
            .query(&[("grant_type", "password")])
            .header("apikey", &self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        let session = response.json::<Session>().await?;

        self.set_auth_state(session.clone()).await;

        Ok(session)
    }

    async fn refresh_login(&self) -> Result<()> {
        let auth_state = self.session.lock().await.clone();

        if let Some(auth_state) = auth_state {
            let now_epoch = now_as_epoch()?;

            // Refresh 1 minute before expiration
            let expired = auth_state.expires_at < now_epoch + 60;

            if expired {
                match self
                    .client
                    .post(format!("{}/auth/v1/token", self.url))
                    .query(&[("grant_type", "refresh_token")])
                    .header("apikey", &self.api_key)
                    .json(&serde_json::json!({
                        "refresh_token": auth_state.refresh_token,
                    }))
                    .send()
                    .await?
                    .error_for_status()
                {
                    Ok(response) => {
                        let token = response.json().await?;

                        self.set_auth_state(token).await;
                    }
                    Err(error) => {
                        if let Some(status) = error.status() {
                            if status == reqwest::StatusCode::BAD_REQUEST {
                                self.session.lock().await.take();
                                return Err(SupabaseError::SessionRefresh(error));
                            }
                        }
                        return Err(error.into());
                    }
                }
            }
            Ok(())
        } else {
            Err(SupabaseError::MissingAuthenticationInformation)
        }
    }

    pub async fn logout(&self) -> Result<()> {
        self.refresh_login().await?;

        self.client
            .post(format!("{}/auth/v1/logout", self.url))
            .query(&[("scope", "local")])
            .header("apikey", &self.api_key)
            .send()
            .await?
            .error_for_status()?;

        self.session.lock().await.take();

        Ok(())
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
