//! Model identity constants. See spec §5.3.

pub const MODEL_ID: &str = "paraphrase-multilingual-MiniLM-L12-v2";
pub const MODEL_DIM: usize = 384;
pub const MAX_SEQ_LEN: usize = 256;

pub const BUNDLE_FILENAME: &str = "mcporb-embed-mmlm12-v1.tar.zst";

/// SHA256 of the bundle tar.zst. Must be updated whenever the bundle changes.
pub const MODEL_TAR_SHA256: &str =
    "0230153f7e955826b9ed629806464a6a7d00f2629e71c22148e49f36b603d487";

pub const MODEL_URLS: &[&str] = &[
    "https://huggingface.co/mcporb/embed-models/resolve/main/mcporb-embed-mmlm12-v1.tar.zst",
    "https://github.com/mcporb/mcporb-dist/releases/download/embed-v1/mcporb-embed-mmlm12-v1.tar.zst",
];
