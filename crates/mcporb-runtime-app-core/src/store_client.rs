use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const STORE_BASE_URL: &str = "https://mcporb.com/api/client/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreOrb {
    pub slug: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub has_password: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSearchResult {
    pub orbs: Vec<StoreOrb>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreOrbVersion {
    pub version: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreDownloadToken {
    pub token: String,
    pub expires_at: String,
    pub download_url: String,
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
    ) -> Result<StoreSearchResult> {
        let url = format!("{}/catalog", self.base_url);
        let resp = self
            .client
            .get(&url)
            .query(&[
                ("q", query),
                ("page", &page.to_string()),
                ("per_page", &per_page.to_string()),
            ])
            .send()
            .await
            .context("failed to send search request")?;

        resp.json::<StoreSearchResult>()
            .await
            .context("failed to parse search response")
    }

    pub async fn get_orb(&self, slug: &str) -> Result<StoreOrb> {
        let url = format!("{}/orbs/{}", self.base_url, slug);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to send get orb request")?;

        resp.json::<StoreOrb>()
            .await
            .context("failed to parse orb response")
    }

    pub async fn get_orb_versions(&self, slug: &str) -> Result<Vec<StoreOrbVersion>> {
        let url = format!("{}/catalog/{}/versions", self.base_url, slug);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("failed to send versions request")?;

        resp.json::<Vec<StoreOrbVersion>>()
            .await
            .context("failed to parse versions response")
    }

    pub async fn verify_download_password(
        &self,
        artifact_id: &str,
        password: &str,
    ) -> Result<StoreDownloadToken> {
        let url = format!("{}/downloads/{}/verify-password", self.base_url, artifact_id);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "password": password }))
            .send()
            .await
            .context("failed to send verify password request")?;

        resp.json::<StoreDownloadToken>()
            .await
            .context("failed to parse download token response")
    }

    pub async fn download_orb(
        &self,
        artifact_id: &str,
        token: &str,
        dest_path: &std::path::Path,
    ) -> Result<u64> {
        let url = format!("{}/downloads/{}", self.base_url, artifact_id);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
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
