//! Manages Orb child process lifecycle: spawn, forward requests, idle-kill.
//!
//! Each Orb runs as a separate `mcporb-runtime --stdio-only` child process.
//! The manager:
//! - Lazily spawns child processes on first request to an Orb
//! - Forwards JSON-RPC requests via the child's stdin/stdout
//! - Tracks last-use timestamps for idle reaping
//! - Automatically reaps child processes after `idle_timeout_secs`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin};
use tokio::sync::RwLock;
use tracing;

use crate::registry_reader::{GatewayConfig, GatewayOrb};

/// Status of an Orb in the runtime manager.
#[derive(Debug)]
enum OrbStatus {
    /// Registered but no child process running.
    Idle,
    /// Child process is running and ready.
    Running(OrbProcess),
    /// The last spawn attempt failed.
    Failed(String),
}

/// A running `mcporb-runtime` child process.
#[derive(Debug)]
struct OrbProcess {
    #[allow(dead_code)]
    slug: String,
    child: Child,
    stdin: BufWriter<ChildStdin>,
    /// Background reader task handle (reads child's stdout).
    #[allow(dead_code)]
    reader_handle: tokio::task::JoinHandle<()>,
    /// Receiver for responses matched by JSON-RPC id.
    response_rx: tokio::sync::mpsc::Receiver<Value>,
    last_used: Instant,
    next_id: u64,
}

/// The `RuntimeManager` is the central coordinator for Orb child processes.
///
/// Thread-safe: all mutable state is behind `RwLock`.
pub struct RuntimeManager {
    config: GatewayConfig,
    /// Known Orbs (from registry). Immutable after construction; re-read
    /// registry explicitly if needed.
    orbs: Vec<GatewayOrb>,
    /// Map from slug to runtime status.
    processes: RwLock<HashMap<String, OrbStatus>>,
    /// Global monotonically increasing request id for gateway-internal use.
    next_gw_id: AtomicU64,
}

impl std::fmt::Debug for RuntimeManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeManager")
            .field("config", &self.config)
            .field("orbs_count", &self.orbs.len())
            .finish()
    }
}

impl RuntimeManager {
    /// Create a new `RuntimeManager`.
    pub fn new(config: GatewayConfig, orbs: Vec<GatewayOrb>) -> Self {
        let mut processes = HashMap::new();
        for orb in &orbs {
            processes.insert(orb.slug.clone(), OrbStatus::Idle);
        }
        Self {
            config,
            orbs,
            processes: RwLock::new(processes),
            next_gw_id: AtomicU64::new(0),
        }
    }

    /// Re-read the registry and update the orb list + process map.
    ///
    /// Called when we suspect the registry has changed. Preserves running
    /// processes for slugs that still exist.
    pub async fn refresh_registry(&mut self) -> Result<()> {
        let new_orbs = crate::registry_reader::discover_orbs(&self.config)?;

        let mut processes = self.processes.write().await;
        let mut new_processes: HashMap<String, OrbStatus> = HashMap::new();

        for orb in &new_orbs {
            let status = processes
                .remove(&orb.slug)
                .unwrap_or(OrbStatus::Idle);
            new_processes.insert(orb.slug.clone(), status);
        }

        // Any remaining entries in `processes` are for deleted Orbs.
        // Killing orphaned processes is handled by Drop (kill_on_drop).
        for (slug, _status) in processes.iter() {
            tracing::info!(orb = %slug, "Tracking orphaned process for removed Orb");
        }

        self.orbs = new_orbs;
        *processes = new_processes;
        Ok(())
    }

    /// Return the list of known Orbs.
    pub fn list_orbs(&self) -> &[GatewayOrb] {
        &self.orbs
    }

    /// Find an Orb by slug.
    pub fn find_orb(&self, slug: &str) -> Option<&GatewayOrb> {
        self.orbs.iter().find(|o| o.slug == slug)
    }

    /// Return the list of all known Orbs with their status.
    pub async fn list_orb_statuses(&self) -> Vec<(GatewayOrb, &'static str)> {
        let processes = self.processes.read().await;
        self.orbs
            .iter()
            .map(|orb| {
                let status = match processes.get(&orb.slug) {
                    Some(OrbStatus::Running(_)) => "running",
                    Some(OrbStatus::Failed(_)) => "failed",
                    _ => "idle",
                };
                (orb.clone(), status)
            })
            .collect()
    }

    /// Ensure the Orb's child process is running. If it's already running,
    /// this is a no-op.
    pub async fn ensure_orb(&self, slug: &str) -> Result<()> {
        let mut processes = self.processes.write().await;
        let status = processes.get(slug);

        match status {
            Some(OrbStatus::Running(_)) => return Ok(()),
            Some(OrbStatus::Failed(msg)) => {
                // Previous spawn failed; try again by resetting to Idle
                tracing::info!(orb = %slug, previous_error = %msg, "Retrying Orb spawn");
            }
            Some(OrbStatus::Idle) => {} // proceed to spawn
            None => {
                anyhow::bail!("Unknown Orb: {slug}");
            }
        }

        // Find the orb config
        let orb = self
            .orbs
            .iter()
            .find(|o| o.slug == slug)
            .ok_or_else(|| anyhow::anyhow!("Unknown Orb: {slug}"))?;

        // Spawn the child process
        match spawn_orb_process(&self.config.runtime_binary, orb, &self.config).await {
            Ok(proc) => {
                tracing::info!(orb = %slug, "Orb process started");
                processes.insert(slug.to_string(), OrbStatus::Running(proc));
                Ok(())
            }
            Err(e) => {
                let msg = format!("Failed to spawn Orb {slug}: {e}");
                tracing::error!(%msg);
                processes.insert(slug.to_string(), OrbStatus::Failed(msg.clone()));
                anyhow::bail!(msg);
            }
        }
    }

    /// Forward a JSON-RPC request to the Orb's child process and return the response.
    ///
    /// The `method` and `params` should already be stripped of namespace prefix.
    pub async fn forward_request(
        &self,
        slug: &str,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        // Ensure the process is running
        self.ensure_orb(slug).await?;

        let processes = self.processes.read().await;
        let status = processes
            .get(slug)
            .ok_or_else(|| anyhow::anyhow!("Unknown Orb: {slug}"))?;

        match status {
            OrbStatus::Running(_proc) => {
                // We need mutable access to the proc. But we only have a read lock,
                // so we drop the read lock and acquire a write lock via the inner fn.
                drop(processes);
                self.forward_request_inner(slug, method, params).await
            }
            OrbStatus::Failed(msg) => {
                anyhow::bail!("Orb {slug} is in a failed state: {msg}")
            }
            OrbStatus::Idle => {
                anyhow::bail!("Orb {slug} is idle (race condition)")
            }
        }
    }

    /// Internal forwarding that acquires a write lock on the process entry.
    async fn forward_request_inner(
        &self,
        slug: &str,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        // To get mutable access to OrbProcess, we need a write lock.
        let mut processes = self.processes.write().await;
        let status = processes
            .get_mut(slug)
            .ok_or_else(|| anyhow::anyhow!("Unknown Orb: {slug}"))?;

        match status {
            OrbStatus::Running(proc) => {
                let request_id = proc.next_id;
                proc.next_id += 1;

                let request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": method,
                    "params": params,
                });

                let request_str = serde_json::to_string(&request)
                    .context("failed to serialize JSON-RPC request")?;

                // Write to child's stdin
                proc.stdin
                    .write_all(request_str.as_bytes())
                    .await
                    .context("failed to write to Orb child stdin")?;
                proc.stdin
                    .write_all(b"\n")
                    .await
                    .context("failed to write newline to Orb child stdin")?;
                proc.stdin
                    .flush()
                    .await
                    .context("failed to flush Orb child stdin")?;

                proc.last_used = Instant::now();
                tracing::debug!(
                    orb = %slug,
                    request_id,
                    method = %method,
                    "Request forwarded to Orb"
                );

                // Wait for the response with a timeout
                let response = tokio::time::timeout(
                    Duration::from_secs(60),
                    proc.response_rx.recv(),
                )
                .await
                .context("timeout waiting for Orb response")?
                .ok_or_else(|| anyhow::anyhow!("Orb child process stopped responding"))?;

                Ok(response)
            }
            OrbStatus::Failed(msg) => {
                anyhow::bail!("Orb {slug} is in a failed state: {msg}")
            }
            OrbStatus::Idle => {
                anyhow::bail!("Orb {slug} is idle (race condition)")
            }
        }
    }

    /// Kill an Orb's child process and reset to Idle.
    pub async fn kill(&self, slug: &str) {
        let mut processes = self.processes.write().await;
        if let Some(OrbStatus::Running(proc)) = processes.get_mut(slug) {
            tracing::info!(orb = %slug, "Killing Orb process");
            let _ = proc.child.kill().await;
            let _ = proc.child.wait().await;
        }
        processes.insert(slug.to_string(), OrbStatus::Idle);
    }

    /// Start the idle reaper background task. Runs every `check_interval_secs`
    /// and kills processes that have been idle longer than `idle_timeout_secs`.
    pub async fn start_reaper(self: Arc<Self>) {
        let check_interval = Duration::from_secs(self.config.check_interval_secs);
        let idle_timeout = Duration::from_secs(self.config.idle_timeout_secs);

        loop {
            tokio::time::sleep(check_interval).await;

            let mut processes = self.processes.write().await;
            let mut to_kill: Vec<String> = Vec::new();

            for (slug, status) in processes.iter() {
                if let OrbStatus::Running(proc) = status {
                    if proc.last_used.elapsed() > idle_timeout {
                        to_kill.push(slug.clone());
                    }
                }
            }

            for slug in &to_kill {
                if let Some(OrbStatus::Running(proc)) = processes.get_mut(slug) {
                    tracing::info!(orb = %slug, idle_secs = ?idle_timeout.as_secs(), "Recycling idle Orb process");
                    let _ = proc.child.kill().await;
                    let _ = proc.child.wait().await;
                }
                processes.insert(slug.clone(), OrbStatus::Idle);
            }

            if !to_kill.is_empty() {
                tracing::info!(count = to_kill.len(), "Orb processes recycled");
            }
        }
    }

    /// Shutdown all child processes gracefully.
    pub async fn shutdown(&self) {
        let mut processes = self.processes.write().await;
        for (slug, status) in processes.iter_mut() {
            if let OrbStatus::Running(proc) = status {
                tracing::info!(orb = %slug, "Shutting down Orb process");
                let _ = proc.child.kill().await;
                let _ = proc.child.wait().await;
            }
        }
        // Reset all to Idle
        for (_, status) in processes.iter_mut() {
            if let OrbStatus::Running(_) = status {
                // Already killed above; this is just a safety net.
                *status = OrbStatus::Idle;
            }
        }
        // Double-check: any remaining Running entries get set to Idle too
        let slugs: Vec<String> = processes.keys().cloned().collect();
        for slug in slugs {
            processes.entry(slug).and_modify(|s| {
                if matches!(s, OrbStatus::Running(_) | OrbStatus::Failed(_)) {
                    *s = OrbStatus::Idle;
                }
            });
        }
    }

    /// Check if an Orb slug exists in our registry.
    pub fn has_orb(&self, slug: &str) -> bool {
        self.orbs.iter().any(|o| o.slug == slug)
    }

    /// Generate the gateway internal request ID.
    pub fn next_id(&self) -> u64 {
        self.next_gw_id.fetch_add(1, Ordering::Relaxed)
    }
}

impl Drop for RuntimeManager {
    fn drop(&mut self) {
        // We can't do async in Drop, so we just log.
        // Child processes will be orphaned — the user should call shutdown().
        tracing::debug!("RuntimeManager dropped — child processes may be orphaned");
    }
}

/// Spawn an `mcporb-runtime` child process for the given Orb.
async fn spawn_orb_process(
    runtime_binary: &PathBuf,
    orb: &GatewayOrb,
    config: &crate::registry_reader::GatewayConfig,
) -> Result<OrbProcess> {
    let metrics_dir = config.registry_dir.join("metrics");
    let mut child = tokio::process::Command::new(runtime_binary)
        .arg("--orb-zip")
        .arg(&orb.zip_path)
        .arg("--stdio-only")
        .arg("--orb-id")
        .arg(&orb.id)
        .arg("--metrics-dir")
        .arg(&metrics_dir)
        .arg("--mcp-transport")
        .arg(&config.mcp_transport)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn mcporb-runtime process")?;

    let stdin = child
        .stdin
        .take()
        .context("failed to capture child stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture child stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture child stderr")?;

    // Forward stderr to tracing logs
    let slug_for_stderr = orb.slug.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::debug!(orb = %slug_for_stderr, stderr = %line);
        }
    });

    let slug = &orb.slug;

    // Response channel: child stdout → mpsc receiver
    let (response_tx, mut response_rx) = tokio::sync::mpsc::channel::<Value>(64);

    // Background task: read stdout lines and forward to response channel
    let slug_clone = orb.slug.clone();
    let reader_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            match serde_json::from_str::<Value>(&line) {
                Ok(response) => {
                    tracing::trace!(orb = %slug_clone, response = %line, "Received response from Orb");
                    if response_tx.send(response).await.is_err() {
                        tracing::warn!(orb = %slug_clone, "Response channel closed");
                        break;
                    }
                }
                Err(e) => {
                    tracing::warn!(orb = %slug_clone, line = %line, error = %e, "Failed to parse Orb response");
                }
            }
        }
        tracing::debug!(orb = %slug_clone, "Orb stdout reader ended");
    });

    // Perform the MCP initialize handshake
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0u64,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "mcporb-gateway",
                "version": "0.1.0"
            }
        }
    });

    let mut stdin_buf = BufWriter::new(stdin);

    let init_str = serde_json::to_string(&init_req)?;
    stdin_buf.write_all(init_str.as_bytes()).await?;
    stdin_buf.write_all(b"\n").await?;
    stdin_buf.flush().await?;

    // Wait for initialize response (first line from child)
    // We'll receive it via our response_rx
    tokio::time::timeout(Duration::from_secs(10), response_rx.recv())
        .await
        .context("timeout waiting for Orb initialize response")?
        .ok_or_else(|| anyhow::anyhow!("Orb child closed stdout during init"))?;

    // Send notifications/initialized (no response expected)
    let notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let notif_str = serde_json::to_string(&notif)?;
    stdin_buf.write_all(notif_str.as_bytes()).await?;
    stdin_buf.write_all(b"\n").await?;
    stdin_buf.flush().await?;

    tracing::info!(orb = %slug, pid = ?child.id().unwrap_or(0), "Orb child process initialized");

    Ok(OrbProcess {
        slug: slug.clone(),
        child,
        stdin: stdin_buf,
        reader_handle,
        response_rx,
        last_used: Instant::now(),
        next_id: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GatewayTool;

    /// Create a minimal GatewayOrb for testing.
    fn test_orb(slug: &str) -> GatewayOrb {
        GatewayOrb {
            id: format!("{slug}_id"),
            slug: slug.to_string(),
            display_name: slug.to_string(),
            description: format!("{slug} test orb"),
            zip_path: PathBuf::from(format!("/tmp/orbs/{slug}.zip")),
            mcp_protocol_version: "2024-11-05".to_string(),
            tools: vec![GatewayTool {
                original_name: "search_knowledge".to_string(),
                namespaced_name: format!("{slug}__search_knowledge"),
                description: "Search tool".to_string(),
                input_schema: serde_json::json!({}),
            }],
        }
    }

    #[tokio::test]
    async fn manager_initializes_with_orbs() {
        let config = GatewayConfig::default();
        let orbs = vec![test_orb("orb-a"), test_orb("orb-b")];
        let manager = RuntimeManager::new(config, orbs);

        assert_eq!(manager.list_orbs().len(), 2);
        assert!(manager.has_orb("orb-a"));
        assert!(manager.has_orb("orb-b"));
        assert!(!manager.has_orb("orb-c"));
    }

    #[tokio::test]
    async fn unknown_orb_ensure_fails() {
        let config = GatewayConfig::default();
        let orbs = vec![test_orb("orb-a")];
        let manager = RuntimeManager::new(config, orbs);

        let result = manager.ensure_orb("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown Orb"));
    }

    #[tokio::test]
    async fn forward_to_unknown_orb_fails() {
        let config = GatewayConfig::default();
        let orbs = vec![test_orb("orb-a")];
        let manager = RuntimeManager::new(config, orbs);

        let result = manager
            .forward_request("nonexistent", "tools/call", serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn list_orbs_status_all_idle_initially() {
        let config = GatewayConfig::default();
        let orbs = vec![test_orb("orb-a"), test_orb("orb-b")];
        let manager = RuntimeManager::new(config, orbs);

        let rt = tokio::runtime::Runtime::new().unwrap();
        let statuses = rt.block_on(manager.list_orb_statuses());

        assert_eq!(statuses.len(), 2);
        for (_, status) in &statuses {
            assert_eq!(*status, "idle");
        }
    }

    #[tokio::test]
    async fn kill_idle_orb_is_noop() {
        let config = GatewayConfig::default();
        let orbs = vec![test_orb("orb-a")];
        let manager = RuntimeManager::new(config, orbs);

        // Killing an idle orb should not error
        manager.kill("orb-a").await;
        // Still idle
        let statuses = manager.list_orb_statuses().await;
        assert_eq!(statuses[0].1, "idle");
    }
}
