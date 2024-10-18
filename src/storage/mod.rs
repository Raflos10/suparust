pub mod object;

use crate::Supabase;

impl Supabase {
    /// Gives you an authenticated [`Storage`] client meant for making one storage request. For multiple
    /// requests, call this function each time.
    ///
    /// This interface is modeled after the definitions [here](https://supabase.github.io/storage/),
    /// but is not yet complete.
    pub async fn storage(&self) -> Storage {
        let url_base = format!("{}/storage/v1", self.url_base);
        let access_token = self
            .session
            .read()
            .await
            .as_ref()
            .map(|session| session.access_token.clone());

        Storage {
            client: AuthenticatedClient {
                client: self.storage_client.clone(),
                access_token,
                apikey: self.api_key.clone(),
            },
            url_base,
        }
    }
}

struct AuthenticatedClient {
    client: reqwest::Client,
    access_token: Option<String>,
    apikey: String,
}

pub struct Storage {
    client: AuthenticatedClient,
    url_base: String,
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
