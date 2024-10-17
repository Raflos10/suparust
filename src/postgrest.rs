use crate::Result;
use crate::Supabase;

use crate::external::postgrest_rs as postgrest;

impl Supabase {
    pub async fn from<T>(&self, table: T) -> Result<postgrest::Builder>
    where
        T: AsRef<str>,
    {
        self.refresh_login().await?;

        Ok(self.postgrest.read().await.from(table))
    }

    pub async fn rpc<T, U>(&self, function: T, params: U) -> Result<postgrest::Builder>
    where
        T: AsRef<str>,
        U: Into<String>,
    {
        self.refresh_login().await?;

        Ok(self.postgrest.read().await.rpc(function, params))
    }
}
