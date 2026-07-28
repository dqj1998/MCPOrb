use serde::{Deserialize, Serialize};
use std::path::Path;

/// Mirrors the Metrics struct from mcporb-runtime state (for file deserialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedMetrics {
    total_requests: u64,
    stdio_requests: u64,
    http_requests: u64,
    total_searches: u64,
    qa_history: Vec<QaEntry>,
}

/// Mirrors the QaEntry struct from mcporb-runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaEntry {
    pub timestamp: String,
    pub query: String,
    pub response_preview: String,
    pub method: String,
    pub num_results: usize,
    pub transport: String,
}

/// Summary metrics for display.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrbMetricsSummary {
    pub total_requests: u64,
    pub stdio_requests: u64,
    pub http_requests: u64,
}

/// Paginated Q&A history response from the Orb's HTTP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaHistoryResponse {
    pub items: Vec<QaEntry>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub total_pages: usize,
}

/// Read metrics summary from a persisted metrics file.
/// Returns `None` if the file does not exist or cannot be parsed.
pub fn read_metrics_from_file(path: &Path) -> Option<OrbMetricsSummary> {
    let data = std::fs::read_to_string(path).ok()?;
    let persisted: PersistedMetrics = serde_json::from_str(&data).ok()?;
    Some(OrbMetricsSummary {
        total_requests: persisted.total_requests,
        stdio_requests: persisted.stdio_requests,
        http_requests: persisted.http_requests,
    })
}

/// Read paginated Q&A history from a persisted metrics file.
pub fn read_qa_history_from_file(
    path: &Path,
    page: usize,
    page_size: usize,
) -> Option<QaHistoryResponse> {
    let data = std::fs::read_to_string(path).ok()?;
    let persisted: PersistedMetrics = serde_json::from_str(&data).ok()?;
    let total = persisted.qa_history.len();
    if total == 0 {
        return Some(QaHistoryResponse {
            items: vec![],
            page,
            page_size,
            total: 0,
            total_pages: 1,
        });
    }
    let total_pages = total.div_ceil(page_size).max(1);
    // Reverse so newest entries (stored at end) appear first on page 1
    let start = (page.saturating_sub(1) * page_size).min(total);
    let end = (start + page_size).min(total);
    let reversed: Vec<_> = persisted.qa_history.into_iter().rev().collect();
    let items = reversed[start..end].to_vec();
    Some(QaHistoryResponse {
        items,
        page,
        page_size,
        total,
        total_pages,
    })
}

/// Fetch metrics summary from a running Orb's HTTP API.
/// Returns `None` if the Orb is not reachable.
pub async fn fetch_orb_metrics(port: u16, token: &str) -> Result<Option<OrbMetricsSummary>, String> {
    let url = format!("http://127.0.0.1:{port}/{token}/api/metrics");
    match reqwest::get(&url).await {
        Ok(resp) if resp.status().is_success() => match resp.json::<OrbMetricsSummary>().await {
            Ok(metrics) => Ok(Some(metrics)),
            Err(e) => Err(format!("failed to parse metrics: {e}")),
        },
        Ok(resp) if resp.status() == reqwest::StatusCode::UNAUTHORIZED => {
            Ok(Some(OrbMetricsSummary {
                ..Default::default()
            }))
        }
        Ok(_) => Ok(None),
        Err(_) => Ok(None),
    }
}

/// Fetch paginated Q&A history from a running Orb's HTTP API.
pub async fn fetch_orb_qa_history(
    port: u16,
    token: &str,
    page: usize,
    page_size: usize,
) -> Result<Option<QaHistoryResponse>, String> {
    let url = format!("http://127.0.0.1:{port}/{token}/api/metrics/qa");
    let client = reqwest::Client::new();
    match client
        .post(&url)
        .json(&serde_json::json!({ "page": page, "page_size": page_size }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<QaHistoryResponse>().await {
            Ok(history) => Ok(Some(history)),
            Err(e) => Err(format!("failed to parse Q&A history: {e}")),
        },
        Ok(_) => Ok(None),
        Err(_) => Ok(None),
    }
}
