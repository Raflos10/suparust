mod postgrest;

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

extern crate postgrest as external_postgrest;

pub type Result<Type> = std::result::Result<Type, SupabaseError>;

#[derive(Clone)]
pub struct Supabase {
    client: reqwest::Client,
    url: String,
    api_key: String,
    auth_state: Arc<Mutex<Option<SupabaseAuthState>>>,
    postgrest: Arc<RwLock<external_postgrest::Postgrest>>,
}

pub type RefreshToken = String;

#[derive(Clone, serde::Deserialize)]
struct AuthToken {
    access_token: String,
    expires_at: i64,
}

#[derive(Clone, serde::Deserialize)]
struct SupabaseAuthState {
    auth_token: Option<AuthToken>,
    refresh_token: RefreshToken,
}

#[derive(thiserror::Error, Debug)]
pub enum SupabaseError {
    #[error("Failed to refresh session: {0}")]
    SessionRefresh(reqwest::Error),
    #[error("Missing authentication information")]
    MissingAuthenticationInformation,
    #[error("Request failed")]
    Reqwest(#[from] reqwest::Error),
}

impl Supabase {
    pub fn new(url: String, api_key: String, refresh_token: Option<RefreshToken>) -> Self {
        let auth_state = refresh_token.map(|refresh_token| SupabaseAuthState {
            auth_token: None,
            refresh_token,
        });

        let postgrest = Arc::new(RwLock::new(
            external_postgrest::Postgrest::new(url.clone())
                .insert_header("apikey", api_key.clone()),
        ));

        Self {
            client: reqwest::Client::new(),
            url,
            api_key,
            auth_state: Arc::new(Mutex::new(auth_state)),
            postgrest,
        }
    }

    async fn set_auth_state(&self, auth_state: SupabaseAuthState) {
        *self.auth_state.lock().await = Some(auth_state.clone());
        if let Some(auth_token) = auth_state.auth_token {
            let mut postgrest = self.postgrest.write().await;
            let authorized_postgrest = postgrest.clone().insert_header(
                "Authorization",
                format!("Bearer {}", auth_token.access_token),
            );
            *postgrest = authorized_postgrest;
        }
    }

    pub async fn has_valid_auth_state(&self) -> bool {
        self.auth_state.lock().await.is_some()
    }

    pub async fn authorize(&self, email: String, password: String) -> Result<RefreshToken> {
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

        #[derive(serde::Deserialize)]
        struct SupabaseAuthResponse {
            pub access_token: String,
            pub expires_at: i64,
            pub refresh_token: String,
        }

        let token = response.json::<SupabaseAuthResponse>().await?;

        let auth_state = SupabaseAuthState {
            auth_token: Some(AuthToken {
                access_token: token.access_token,
                expires_at: token.expires_at,
            }),
            refresh_token: token.refresh_token.clone(),
        };
        self.set_auth_state(auth_state).await;

        Ok(token.refresh_token)
    }

    async fn refresh_login(&self) -> Result<()> {
        let auth_state = self.auth_state.lock().await.clone();

        if let Some(auth_state) = auth_state {
            let now_epoch = now_as_epoch();

            let expired = {
                if let Some(auth_token) = &auth_state.auth_token {
                    // Refresh 1 minute before expiration
                    auth_token.expires_at < now_epoch + 60
                } else {
                    true
                }
            };

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
                                self.auth_state.lock().await.take();
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

        self.auth_state.lock().await.take();

        Ok(())
    }
}

#[cfg(target_family = "wasm")]
fn now_as_epoch() -> i64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)?
        .as_secs() as i64
}
#[cfg(not(target_family = "wasm"))]
fn now_as_epoch() -> i64 {
    chrono::Utc::now().timestamp()
}
