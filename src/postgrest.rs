//! Holds some types that you will get from using the Supabase client. These types are not meant to
//! be used directly.

use crate::Result;
use crate::Supabase;

use crate::external::postgrest_rs as postgrest;
pub use postgrest::Builder;

impl Supabase {
    /// A wrapper for `postgrest::Postgrest::from` that gives you an already authenticated [`Builder`]
    pub async fn from<T>(&self, table: T) -> Result<Builder>
    where
        T: AsRef<str>,
    {
        self.refresh_login().await?;

        Ok(self.postgrest.read().await.from(table))
    }

    /// A wrapper for `postgrest::Postgrest::rpc` that gives you an already authenticated [`Builder`]
    pub async fn rpc<T, U>(&self, function: T, params: U) -> Result<Builder>
    where
        T: AsRef<str>,
        U: Into<String>,
    {
        self.refresh_login().await?;

        Ok(self.postgrest.read().await.rpc(function, params))
    }
}
