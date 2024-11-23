pub mod object;

use crate::Supabase;

impl Supabase {
    /// Gives you an authenticated [`Storage`] client meant for making one storage request. For multiple
    /// requests, call this function each time.
    ///
    /// This interface is modeled after the definitions [here](https://supabase.github.io/storage/),
    /// but is not yet complete.
    pub async fn storage(&self) -> crate::Result<Storage> {
        let url_base = format!("{}/storage/v1", self.url_base);

        self.refresh_login().await?;

        let access_token = self
            .session
            .read()
            .await
            .as_ref()
            .map(|session| session.access_token.clone());

        Ok(Storage {
            client: AuthenticatedClient {
                client: self.storage_client.clone(),
                access_token,
                apikey: self.api_key.clone(),
            },
            url_base,
        })
    }
}

#[derive(Debug)]
struct AuthenticatedClient {
    client: reqwest::Client,
    access_token: Option<String>,
    apikey: String,
}

#[derive(Debug)]
pub struct Storage {
    client: AuthenticatedClient,
    url_base: String,
}

/// errorSchema as defined under schemas at [the api documentation](https://supabase.github.io/storage/)
#[derive(
    Debug,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Hash,
    Default,
    serde::Deserialize,
    thiserror::Error,
)]
pub struct Error {
    #[serde(rename = "statusCode")]
    pub status_code: String,
    pub error: String,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Storage {
    /// Object end-points
    pub fn object(self) -> object::Object {
        object::Object {
            client: self.client,
            url_base: format!("{}/object", self.url_base),
        }
    }
}

trait AuthenticateClient {
    fn authenticate(self, authenticator: &AuthenticatedClient) -> reqwest::RequestBuilder;
}

impl AuthenticateClient for reqwest::RequestBuilder {
    fn authenticate(self, authenticator: &AuthenticatedClient) -> reqwest::RequestBuilder {
        match &authenticator.access_token {
            Some(access_token) => self.header("Authorization", format!("Bearer {}", access_token)),
            None => self,
        }
        .header("apikey", authenticator.apikey.clone())
    }
}

trait DecodeStorageErrorResponse {
    async fn decode_storage_error_response(self) -> crate::Result<reqwest::Response>;
}

impl DecodeStorageErrorResponse for reqwest::Response {
    async fn decode_storage_error_response(self) -> crate::Result<reqwest::Response> {
        let status = self.status();
        if status.is_client_error() || status.is_server_error() {
            let error = self.json::<Error>().await?;
            Err(error.into())
        } else {
            Ok(self)
        }
    }
}

trait SendAndDecodeStorageRequest<Type> {
    async fn send_and_decode_storage_request(self) -> crate::Result<Type>;
}

impl<Type> SendAndDecodeStorageRequest<Type> for reqwest::RequestBuilder
where
    Type: serde::de::DeserializeOwned,
{
    async fn send_and_decode_storage_request(self) -> crate::Result<Type> {
        Ok(self
            .send()
            .await?
            .decode_storage_error_response()
            .await?
            .json()
            .await?)
    }
}
