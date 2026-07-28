use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const STORE_BASE_URL: &str = "https://mcporb.store/api";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreOrb {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub published_at: Option<String>,
    pub methods: Vec<String>,
    pub is_private: bool,
    pub password_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub items: Vec<StoreOrb>,
    pub page: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagInfo {
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrbDetail {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub is_private: bool,
    pub latest_version: String,
    pub latest_version_id: String,
    pub methods: Vec<String>,
    pub tags: Vec<String>,
    pub artifacts: Vec<ArtifactInfo>,
    pub versions: Vec<VersionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub id: String,
    pub kind: String,
    pub size_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub id: String,
    pub version: String,
    pub published_at: Option<String>,
    pub artifacts: Vec<ArtifactInfo>,
    pub has_password: bool,
    pub has_custom_password: Option<bool>,
    pub is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadToken {
    pub token: String,
    pub expires_in_seconds: i64,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TagsResponse {
    List(Vec<TagInfo>),
    Object { tags: Vec<TagInfo> },
}

pub struct StoreClient {
    client: reqwest::Client,
    base_url: String,
}

impl StoreClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            client,
            base_url: STORE_BASE_URL.to_string(),
        })
    }

    pub async fn search_orbs(
        &self,
        query: &str,
        page: usize,
        per_page: usize,
    ) -> Result<ListResponse> {
        let _ = per_page;

        self.search_orbs_filtered(query, None, None, page as i64).await
    }

    pub async fn search_orbs_filtered(
        &self,
        query: &str,
        tag: Option<&str>,
        method: Option<&str>,
        page: i64,
    ) -> Result<ListResponse> {
        let url = format!("{}/orbs", self.base_url);
        let page = page.max(1).to_string();
        let resp = self
            .client
            .get(&url)
            .query(&[("q", query), ("page", page.as_str())])
            .query(&tag.map(|tag| [("tag", tag)]))
            .query(&method.map(|method| [("method", method)]))
            .send()
            .await
            .context("failed to send search request")?;

        resp.json::<ListResponse>()
            .await
            .context("failed to parse search response")
    }

    pub async fn get_orb(&self, slug: &str) -> Result<OrbDetail> {
        let url = format!("{}/orbs/{}", self.base_url, slug);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to send get orb request")?;

        resp.json::<OrbDetail>()
            .await
            .context("failed to parse orb response")
    }

    pub async fn list_tags(&self) -> Result<Vec<TagInfo>> {
        let url = format!("{}/tags", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to send list tags request")?;

        let tags = resp
            .json::<TagsResponse>()
            .await
            .context("failed to parse tags response")?;

        Ok(match tags {
            TagsResponse::List(tags) => tags,
            TagsResponse::Object { tags } => tags,
        })
    }

    pub async fn verify_download_password(
        &self,
        artifact_id: &str,
        password: &str,
    ) -> Result<DownloadToken> {
        let url = format!("{}/downloads/{}/verify-password", self.base_url, artifact_id);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "password": password }))
            .send()
            .await
            .context("failed to send verify password request")?;

        // Check HTTP status before trying to parse JSON so that server error
        // responses (wrong password, expired link, etc.) produce a readable
        // message instead of a cryptic JSON parse failure.
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            // Try to extract a structured error message from the JSON body
            if let Ok(err) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(msg) = err.get("error").and_then(|v| v.as_str()) {
                    return Err(anyhow::anyhow!("{}", msg));
                }
            }
            // Fall back to the raw body or status text
            if !body.is_empty() {
                return Err(anyhow::anyhow!("{}", body));
            }
            return Err(anyhow::anyhow!(
                "request failed with status {}",
                status.as_u16()
            ));
        }

        resp.json::<DownloadToken>()
            .await
            .context("failed to parse server response")
    }

    pub async fn download_orb(
        &self,
        artifact_id: &str,
        token: &str,
        dest_path: &std::path::Path,
    ) -> Result<u64> {
        let url = format!("{}/downloads/{}", self.base_url, artifact_id);
        let mut request = self.client.get(&url);
        if !token.is_empty() {
            request = request
                .header("x-download-token", token)
                .query(&[("download_token", token)]);
        }
        let resp = request
            .send()
            .await
            .context("failed to send download request")?;

        let bytes = resp
            .bytes()
            .await
            .context("failed to read download response")?;

        std::fs::write(dest_path, &bytes)
            .with_context(|| format!("failed to write to {}", dest_path.display()))?;

        Ok(bytes.len() as u64)
    }
}
