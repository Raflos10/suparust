use crate::storage::AuthenticateClient;

pub struct Object {
    pub(super) client: crate::storage::AuthenticatedClient,
    pub(super) url_base: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObjectIdentifier {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Key")]
    pub key: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SortBy {
    pub column: String,
    pub order: SortOrder,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ListRequest {
    pub prefix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(rename = "sortBy")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<SortBy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BucketInformation {
    pub id: String,
    pub name: String,
    pub owner: Option<String>,
    pub public: Option<bool>,
    pub file_size_limit: Option<i64>,
    pub allowed_mime_types: Option<Vec<serde_json::Value>>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ObjectInformation {
    pub name: String,
    pub bucket_id: Option<String>,
    pub owner: Option<String>,
    pub owner_id: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
    pub updated_at: Option<String>,
    pub created_at: Option<String>,
    pub last_accessed_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub user_metadata: Option<serde_json::Value>,
    pub buckets: Option<BucketInformation>,
}

impl ListRequest {
    pub fn new(prefix: String) -> Self {
        Self {
            prefix,
            limit: None,
            offset: None,
            sort_by: None,
            search: None,
        }
    }

    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn sort_by(mut self, column: &str, order: SortOrder) -> Self {
        self.sort_by = Some(SortBy {
            column: column.to_string(),
            order,
        });
        self
    }

    pub fn search(mut self, search: &str) -> Self {
        self.search = Some(search.to_string());
        self
    }
}
impl Object {
    pub async fn delete_one(self, bucket_name: &str, wildcard: &str) -> crate::Result<String> {
        Ok(self
            .client
            .client
            .delete(format!("{}/{bucket_name}/{wildcard}", self.url_base))
            .authenticate(&self.client)
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn get_one(self, bucket_name: &str, wildcard: &str) -> crate::Result<Vec<u8>> {
        Ok(self
            .client
            .client
            .get(format!("{}/{bucket_name}/{wildcard}", self.url_base))
            .authenticate(&self.client)
            .send()
            .await?
            .bytes()
            .await?
            .to_vec())
    }

    pub async fn update_one(
        self,
        bucket_name: &str,
        wildcard: &str,
        data: Vec<u8>,
        content_type: Option<mime::Mime>,
    ) -> crate::Result<ObjectIdentifier> {
        let request = self
            .client
            .client
            .put(format!("{}/{bucket_name}/{wildcard}", self.url_base))
            .authenticate(&self.client)
            .body(data);

        let request = match content_type {
            Some(content_type) => request.header("Content-Type", content_type.to_string()),
            None => request,
        };

        Ok(request.send().await?.json().await?)
    }

    pub async fn upload_one(
        self,
        bucket_name: &str,
        wildcard: &str,
        data: Vec<u8>,
        content_type: Option<mime::Mime>,
    ) -> crate::Result<ObjectIdentifier> {
        let request = self
            .client
            .client
            .post(format!("{}/{bucket_name}/{wildcard}", self.url_base))
            .authenticate(&self.client)
            .body(data);

        let request = match content_type {
            Some(content_type) => request.header("Content-Type", content_type.to_string()),
            None => request,
        };

        Ok(request.send().await?.json().await?)
    }

    pub async fn list(
        self,
        bucket_name: &str,
        request: ListRequest,
    ) -> crate::Result<Vec<ObjectInformation>> {
        Ok(self
            .client
            .client
            .post(format!("{}/list/{bucket_name}", self.url_base))
            .authenticate(&self.client)
            .json(&request)
            .send()
            .await?
            .json()
            .await?)
    }
}
