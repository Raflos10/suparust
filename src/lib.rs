//! # Examples
//!
//! Simple example:
//! ```
//! # pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let client = suparust::Supabase::new(
//!     "https://your.postgrest.endpoint",
//!     "your_api_key",
//!     None,
//!     suparust::SessionChangeListener::Ignore);
//!
//! client.login_with_email(
//!     "myemail@example.com",
//!     "mypassword").await?;
//!
//! #[derive(serde::Deserialize)]
//! struct MyStruct {
//!     id: i64,
//!     field: String
//! }
//!
//! // Postgrest example (see postgrest crate for more details on API)
//! let table_contents = client
//!     .from("your_table")
//!     .await?
//!     .select("*")
//!     .execute()
//!     .await?
//!     .json::<Vec<MyStruct>>();
//!
//! # Ok(())
//! # }

mod auth;
mod postgrest;
pub mod storage;
#[cfg(test)]
mod tests;

use std::sync::Arc;
pub use supabase_auth::models::{LogoutScope, Session, User};
use tokio::sync::RwLock;

extern crate postgrest as external_postgrest;

pub type Result<Type> = std::result::Result<Type, SupabaseError>;

#[derive(Clone)]
pub struct Supabase {
    auth: Arc<supabase_auth::models::AuthClient>,
    session: Arc<RwLock<Option<Session>>>,
    session_listener: SessionChangeListener,
    postgrest: Arc<RwLock<external_postgrest::Postgrest>>,
    storage_client: reqwest::Client,
    api_key: String,
    url_base: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SupabaseError {
    #[error("Failed to refresh session: {0}")]
    SessionRefresh(supabase_auth::error::Error),
    #[error("Missing authentication information")]
    MissingAuthenticationInformation,
    #[error("Request failed")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error from auth layer: {0}")]
    Auth(#[from] supabase_auth::error::Error),
    #[error("Internal error: {0}")]
    Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
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

        let auth = supabase_auth::models::AuthClient::new(url, api_key, "");

        Self {
            auth: Arc::new(auth),
            session: Arc::new(RwLock::new(session)),
            session_listener,
            postgrest: Arc::new(RwLock::new(postgrest)),
            storage_client: Default::default(),
            api_key: api_key.to_string(),
            url_base: url.to_string(),
        }
    }
}
