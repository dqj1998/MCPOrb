//! Model bundle downloader + on-disk cache. See spec §5.4.
//!
//! Pipeline per source URL:
//!   stream into <cache>/.tmp/bundle.tar.zst
//!     → SHA256 verify against [`MODEL_TAR_SHA256`]
//!     → zstd-decompress → tar-extract into <cache>/.staging/
//!     → write `.source_sha256` marker
//!     → atomic rename <cache>/.staging → <cache>/current
//!
//! On any failure: try the next URL. Temp dirs are cleaned up; the previous
//! `current/` (if any) is left untouched.

use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::model::{BUNDLE_FILENAME, MODEL_TAR_SHA256, MODEL_URLS};

/// Files the bundle is allowed to contain. Anything else in the tar is
/// rejected (defense-in-depth against tar path-traversal / unexpected
/// payloads even though SHA256 already gates content).
const ALLOWED_FILES: &[&str] = &[
    "model_f16.onnx",
    "tokenizer.json",
    "special_tokens_map.json",
    "config.json",
];

pub struct ModelManager {
    cache_dir: PathBuf,
}

impl ModelManager {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("cannot resolve home directory"))?
            .join(".mcporb")
            .join("models");
        Ok(Self { cache_dir })
    }

    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn current_dir(&self) -> PathBuf {
        self.cache_dir.join("current")
    }

    pub fn optimized_plan_path(&self, sha: &str) -> PathBuf {
        self.cache_dir
            .join("optimized")
            .join(format!("{}.plan", sha))
    }

    /// Fast readiness check: compares the marker file content against the
    /// compile-time pinned SHA. Avoids re-hashing the 235MB model on every
    /// startup (spec §5.4 rule 1).
    pub fn is_ready(&self) -> bool {
        let marker = self.current_dir().join(".source_sha256");
        match std::fs::read_to_string(&marker) {
            Ok(actual) => actual.trim() == MODEL_TAR_SHA256,
            Err(_) => false,
        }
    }

    /// Synchronous wrapper around [`ModelManager::download`]. Drives the
    /// async download on a dedicated OS thread with its own current-thread
    /// tokio runtime, so it works even when the caller is itself inside a
    /// tokio runtime (e.g. `mcporb-cli`'s `#[tokio::main]`). Used by the
    /// Builder which is otherwise sync.
    pub fn download_blocking(&self) -> Result<&'static str> {
        let cache_dir = self.cache_dir.clone();
        let handle = std::thread::spawn(move || -> Result<&'static str> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("creating blocking tokio runtime for download")?;
            let mm = ModelManager::with_cache_dir(cache_dir);
            rt.block_on(mm.download())
        });
        handle
            .join()
            .map_err(|_| anyhow!("download thread panicked"))?
    }

    /// Try each URL in [`MODEL_URLS`] until one succeeds end-to-end
    /// (download + verify + extract + commit). Returns the URL that worked.
    pub async fn download(&self) -> Result<&'static str> {
        std::fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("creating cache dir {:?}", self.cache_dir))?;

        let mut last_err: Option<anyhow::Error> = None;
        for url in MODEL_URLS {
            tracing::info!(url = %url, "attempting model download");
            match self.try_one(url).await {
                Ok(()) => {
                    tracing::info!(url = %url, "model installed");
                    return Ok(*url);
                }
                Err(e) => {
                    tracing::warn!(url = %url, error = %e, "download attempt failed");
                    last_err = Some(e);
                }
            }
        }
        Err(last_err
            .unwrap_or_else(|| anyhow!("no MODEL_URLS configured"))
            .context("all download sources failed"))
    }

    async fn try_one(&self, url: &str) -> Result<()> {
        // Per-attempt temp / staging dirs (created fresh, cleaned on failure).
        let tmp_dir = self.cache_dir.join(".tmp");
        let staging_dir = self.cache_dir.join(".staging");
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let _ = std::fs::remove_dir_all(&staging_dir);
        std::fs::create_dir_all(&tmp_dir)?;
        std::fs::create_dir_all(&staging_dir)?;

        let bundle_path = tmp_dir.join(BUNDLE_FILENAME);

        let stream_result = stream_to_file_with_hash(url, &bundle_path).await;
        let actual_sha = match stream_result {
            Ok(sha) => sha,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(e);
            }
        };
        if actual_sha != MODEL_TAR_SHA256 {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            let _ = std::fs::remove_dir_all(&staging_dir);
            bail!(
                "SHA256 mismatch (corruption or wrong bundle): expected {}, got {}",
                MODEL_TAR_SHA256,
                actual_sha
            );
        }

        if let Err(e) = extract_bundle(&bundle_path, &staging_dir) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(e.context("extracting tar.zst bundle"));
        }

        std::fs::write(staging_dir.join(".source_sha256"), MODEL_TAR_SHA256)
            .context("writing .source_sha256 marker")?;

        let current = self.current_dir();
        let _ = std::fs::remove_dir_all(&current);
        std::fs::rename(&staging_dir, &current)
            .with_context(|| format!("committing staging -> {:?}", current))?;
        let _ = std::fs::remove_dir_all(&tmp_dir);
        Ok(())
    }
}

/// Stream `url` to `dest`, hashing on the fly. Returns hex SHA256.
async fn stream_to_file_with_hash(url: &str, dest: &Path) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("mcporb-embed/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {}", url))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {}", url))?;

    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("creating temp file {:?}", dest))?;
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("reading response stream")?;
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .context("writing chunk to disk")?;
    }
    file.flush().await?;
    drop(file);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Extract a tar.zst from `bundle_path` into `dest_dir`, accepting only the
/// 4 whitelisted regular files in [`ALLOWED_FILES`]. Rejects any entry with
/// path traversal segments, absolute paths, or symlinks.
fn extract_bundle(bundle_path: &Path, dest_dir: &Path) -> Result<()> {
    let bundle =
        std::fs::File::open(bundle_path).with_context(|| format!("opening {:?}", bundle_path))?;
    let zstd_reader = zstd::stream::read::Decoder::new(bundle).context("creating zstd decoder")?;
    let mut tar = tar::Archive::new(zstd_reader);

    let allowed: HashSet<&str> = ALLOWED_FILES.iter().copied().collect();
    let mut seen: HashSet<String> = HashSet::new();

    for entry in tar.entries().context("reading tar entries")? {
        let mut entry = entry.context("reading tar entry")?;

        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() {
            bail!(
                "rejecting non-regular tar entry type {:?} at {:?}",
                entry_type,
                entry.path().ok()
            );
        }

        let path = entry
            .path()
            .context("decoding tar entry path")?
            .into_owned();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("tar entry has no valid filename: {:?}", path))?;

        // Must be a bare filename — no subdir, no traversal, no absolute.
        if path.components().count() != 1 {
            bail!(
                "rejecting tar entry with multiple path components: {:?}",
                path
            );
        }
        if name.contains("..") || name.starts_with('/') {
            bail!("rejecting suspicious tar entry name: {:?}", name);
        }
        if !allowed.contains(name) {
            bail!("rejecting non-whitelisted tar entry: {:?}", name);
        }
        if !seen.insert(name.to_string()) {
            bail!("duplicate tar entry: {:?}", name);
        }

        // Read into memory then write — small, safe, lets us bound size.
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .context("reading tar entry body")?;
        let out = dest_dir.join(name);
        std::fs::write(&out, &buf).with_context(|| format!("writing {:?}", out))?;
    }

    // Verify all expected files arrived.
    for expected in ALLOWED_FILES {
        if !seen.contains(*expected) {
            bail!("bundle is missing required file: {}", expected);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fresh_manager_is_not_ready() {
        let tmp = tempdir_unique();
        let m = ModelManager::with_cache_dir(tmp.clone());
        assert!(!m.is_ready());
        cleanup(&tmp);
    }

    #[test]
    fn is_ready_true_when_marker_matches() {
        let tmp = tempdir_unique();
        let m = ModelManager::with_cache_dir(tmp.clone());
        std::fs::create_dir_all(m.current_dir()).unwrap();
        std::fs::write(m.current_dir().join(".source_sha256"), MODEL_TAR_SHA256).unwrap();
        assert!(m.is_ready());
        cleanup(&tmp);
    }

    #[test]
    fn is_ready_false_when_marker_stale() {
        let tmp = tempdir_unique();
        let m = ModelManager::with_cache_dir(tmp.clone());
        std::fs::create_dir_all(m.current_dir()).unwrap();
        std::fs::write(m.current_dir().join(".source_sha256"), "stale_sha").unwrap();
        assert!(!m.is_ready());
        cleanup(&tmp);
    }

    #[test]
    fn extract_happy_path() {
        let tmp = tempdir_unique();
        std::fs::create_dir_all(&tmp).unwrap();
        let bundle = tmp.join("bundle.tar.zst");
        let dest = tmp.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_zst(
            &bundle,
            &[
                ("model_f16.onnx", b"fake onnx bytes"),
                ("tokenizer.json", b"{}"),
                ("special_tokens_map.json", b"{}"),
                ("config.json", b"{}"),
            ],
        );

        extract_bundle(&bundle, &dest).expect("extract should succeed");
        assert_eq!(
            std::fs::read(dest.join("model_f16.onnx")).unwrap(),
            b"fake onnx bytes"
        );
        cleanup(&tmp);
    }

    #[test]
    fn extract_rejects_missing_required_file() {
        let tmp = tempdir_unique();
        std::fs::create_dir_all(&tmp).unwrap();
        let bundle = tmp.join("bundle.tar.zst");
        let dest = tmp.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_zst(
            &bundle,
            &[
                ("model_f16.onnx", b"x"),
                ("tokenizer.json", b"x"),
                // missing special_tokens_map.json + config.json
            ],
        );

        let err = extract_bundle(&bundle, &dest).unwrap_err().to_string();
        assert!(
            err.contains("missing required file"),
            "expected missing-file error, got: {}",
            err
        );
        cleanup(&tmp);
    }

    #[test]
    fn extract_rejects_non_whitelisted_file() {
        let tmp = tempdir_unique();
        std::fs::create_dir_all(&tmp).unwrap();
        let bundle = tmp.join("bundle.tar.zst");
        let dest = tmp.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_zst(
            &bundle,
            &[
                ("model_f16.onnx", b"x"),
                ("tokenizer.json", b"x"),
                ("special_tokens_map.json", b"x"),
                ("config.json", b"x"),
                ("evil_backdoor.so", b"\x7fELF..."),
            ],
        );

        let err = extract_bundle(&bundle, &dest).unwrap_err().to_string();
        assert!(
            err.contains("non-whitelisted"),
            "expected whitelist rejection, got: {}",
            err
        );
        cleanup(&tmp);
    }

    #[test]
    fn extract_rejects_subdir_path() {
        let tmp = tempdir_unique();
        std::fs::create_dir_all(&tmp).unwrap();
        let bundle = tmp.join("bundle.tar.zst");
        let dest = tmp.join("dest");
        std::fs::create_dir_all(&dest).unwrap();

        build_tar_zst(
            &bundle,
            &[
                ("model_f16.onnx", b"x"),
                ("tokenizer.json", b"x"),
                ("special_tokens_map.json", b"x"),
                ("subdir/config.json", b"x"),
            ],
        );

        let err = extract_bundle(&bundle, &dest).unwrap_err().to_string();
        assert!(
            err.contains("multiple path components"),
            "expected subdir rejection, got: {}",
            err
        );
        cleanup(&tmp);
    }

    /// Build a tar.zst containing the given (name, content) pairs as regular
    /// files at the archive root. Used only by tests.
    fn build_tar_zst(out_path: &Path, files: &[(&str, &[u8])]) {
        let f = std::fs::File::create(out_path).unwrap();
        let zstd = zstd::stream::write::Encoder::new(f, 3).unwrap();
        let mut tar = tar::Builder::new(zstd);
        for (name, body) in files {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(body.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append(&header, *body).unwrap();
        }
        let zstd = tar.into_inner().unwrap();
        let mut f = zstd.finish().unwrap();
        f.flush().unwrap();
    }

    fn tempdir_unique() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("mcporb-embed-test-{}", uniq()));
        p
    }

    fn cleanup(p: &Path) {
        let _ = std::fs::remove_dir_all(p);
    }

    fn uniq() -> u128 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
