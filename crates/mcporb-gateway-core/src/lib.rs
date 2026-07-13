//! # MCPOrb Gateway Core
//!
//! Aggregate multiple knowledge Orbs behind a single MCP endpoint.
//!
//! ## Architecture
//!
//! The gateway presents a unified MCP interface (STDIO or HTTP) that:
//!
//! 1. **Discovers** installed Orbs from the shared `registry.json`
//! 2. **Aggregates** `tools/list` and `resources/list` across all Orbs
//! 3. **Routes** `tools/call` and `resources/read` to the correct Orb by
//!    namespace prefix (e.g. `my-orb__search_knowledge`)
//! 4. **Manages** Orb child process lifecycle (lazy spawn, idle kill)

pub mod handler;
pub mod registry_reader;
pub mod router;
pub mod runtime_manager;

/// A tool definition for a single Orb, with both original and namespaced names.
#[derive(Debug, Clone)]
pub struct GatewayTool {
    /// The original method name inside the Orb (e.g. `search_knowledge`).
    pub original_name: String,
    /// The namespaced name exposed to MCP clients (e.g. `my-orb__search_knowledge`).
    pub namespaced_name: String,
    /// Human-readable description for the tool.
    pub description: String,
    /// JSON Schema for the tool's arguments.
    pub input_schema: serde_json::Value,
}
