//! # suparust
//!
//! A crate for interacting with Supabase. Also supports WASM targets.
//!
//! ## Usage
//!
//! Create your Supabase client with `Supabase::new` and start using it. The client will automatically
//! handle authentication for you after you have logged in with the client.
//!
//! ### Postgrest
//!
//! Use the functions [`from`](Supabase::from) and [`rpc`](Supabase::rpc) to get a [`postgrest::Builder`] that
//! you can use to build your queries. The builder will automatically have authentication (if it's available)
//! when it's first created.
//!
//! ### Storage
//!
//! Use the function [`storage`](Supabase::storage) to get a one-time-use [`storage::Storage`] client for interacting
//! with the storage part of Supabase. The client will automatically have authentication (if it's available)
//! when it's first created.
//!
//! ### Auth
//!
//! Auth functions are available directly on the Supabase client. Use the functions [`login_with_email`](Supabase::login_with_email),
//! and [`logout`](Supabase::logout) for basic authentication. The client will automatically handle
//! refreshing if needed when making requests.
//!
//! The session refresh happens if it is less than [`auth::SESSION_REFRESH_GRACE_PERIOD_SECONDS`] seconds
//! from expiring. This means that you should not keep authenticated builders/temporary clients for
//! too long before using them, as they might time out.
//!
//! <div class="warning">
//!     Don't keep authenticated builders/clients from postgrest and storage too long, as they might
//!     time out after some time. See details in Auth description above.
//! </div>
//!
//! ## Examples
//!
//! ### Simple postgrest example
//! ```no_run
//! # pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let client = suparust::Supabase::new(
//!     "https://your.postgrest.endpoint",
//!     "your_api_key",
//!     None,
//!     suparust::auth::SessionChangeListener::Ignore);
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
//! ```
//!
//! ### Storage example
//! ```no_run
//! # pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let client = suparust::Supabase::new(
//!     "https://your.postgrest.endpoint",
//!     "your_api_key",
//!     None,
//!     suparust::auth::SessionChangeListener::Ignore);
//!
//! // Login here
//!
//! # use suparust::storage::object::*;
//! let list_request = ListRequest::new("my_folder".to_string())
//!     .limit(10)
//!     .sort_by("my_column", SortOrder::Ascending);
//! let objects = client
//!     .storage()
//!     .await?
//!     .object()
//!     .list("my_bucket", list_request)
//!     .await?;
//!
//! let object_names = objects
//!     .iter()
//!     .map(|object| object.name.clone());
//!
//! let mut downloaded_objects = vec![];
//!
//! for object in objects {
//!     let downloaded = client
//!         .storage()
//!         .await?
//!         .object()
//!         .get_one("my_bucket", &object.name)
//!         .await?;
//!     downloaded_objects.push(downloaded);
//! }
//!
//! # Ok(())
//! # }
//! ```

pub mod auth;
mod external;
pub mod postgrest;
pub mod storage;
#[cfg(test)]
mod tests;

use std::sync::Arc;
use tokio::sync::RwLock;

pub type Result<Type> = std::result::Result<Type, SupabaseError>;

/// The main Supabase client. This is safely cloneable.
#[derive(Clone)]
pub struct Supabase {
    auth: Arc<supabase_auth::models::AuthClient>,
    session: Arc<RwLock<Option<auth::Session>>>,
    session_listener: auth::SessionChangeListener,
    postgrest: Arc<RwLock<external::postgrest_rs::Postgrest>>,
    storage_client: reqwest::Client,
    api_key: String,
    url_base: String,
}

#[derive(thiserror::Error, Debug)]
pub enum SupabaseError {
    /// Failed to refresh session
    #[error("Failed to refresh session: {0}")]
    SessionRefresh(supabase_auth::error::Error),
    /// Missing authentication information. Maybe you are not logged in?
    #[error("Missing authentication information. Maybe you are not logged in?")]
    MissingAuthenticationInformation,
    #[error("Error from storage: {0}")]
    Storage(#[from] storage::Error),
    #[error("Unable to guess MIME type")]
    UnknownMimeType,
    #[error("Request failed")]
    Reqwest(#[from] reqwest::Error),
    #[error("Error from auth layer: {0}")]
    Auth(#[from] supabase_auth::error::Error),
    #[error("Internal error: {0}")]
    Internal(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl Supabase {
    /// Create a new Supabase client
    ///
    /// # Arguments
    /// * `url` - The URL of the Postgrest endpoint
    /// * `api_key` - The API key for the Postgrest endpoint
    /// * `session` - An optional session to use for authentication. This is typically session
    ///     information that is either gotten through the listener (next parameter to this function),
    ///     or externally if you get a valid session from somewhere else (e.g. a magic link).
    /// * `session_listener` - A listener for session changes. This can be used to listen for session
    ///     changes and e.g. update a saved state for use at next run. If you don't need this, you
    ///     can use `SessionChangeListener::Ignore`.
    ///
    /// # Example
    ///
    /// ## Basic usage
    /// ```no_run
    /// # use suparust::*;
    /// let client = Supabase::new(
    ///     "https://your.postgrest.endpoint",
    ///     "your_api_key",
    ///     None,
    ///     auth::SessionChangeListener::Ignore);
    /// ```
    ///
    /// ## Persist session information
    /// ```no_run
    /// # use suparust::*;
    /// # fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    /// # let load_session = || None;
    ///
    /// let loaded_session = load_session(); // Load session from somewhere
    ///
    /// let (sender, receiver) = std::sync::mpsc::channel();
    ///
    /// let client = Supabase::new(
    ///     "https://your.postgrest.endpoint",
    ///     "your_api_key",
    ///     loaded_session,
    ///     auth::SessionChangeListener::Sync(sender));
    ///
    /// let session = receiver.recv()?;
    ///
    /// # let save_session = | _session | ();
    /// save_session(session);
    /// # Ok(())
    /// # }
    pub fn new(
        url: &str,
        api_key: &str,
        session: Option<auth::Session>,
        session_listener: auth::SessionChangeListener,
    ) -> Self {
        let mut postgrest = external::postgrest_rs::Postgrest::new(format!("{url}/rest/v1"))
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
