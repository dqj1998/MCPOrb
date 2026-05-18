# MCPOrb 详细开发计划书

## 1. 项目定位与架构愿景

`MCPOrb` 是一个单二进制、自包含的 MCP 能力包生成生态。用户把 PDF/Markdown 知识库、未来的 OpenAPI 描述或本地脚本交给构建工具，最终得到一个可直接交付的 `Orb` 可执行文件。

每个生成出来的 Orb 同时具备两类入口：

1. **AI 消费入口**：通过 Stdio JSON-RPC 暴露标准 MCP Server 能力，供 Claude Desktop、Cursor、VS Code 等 MCP 客户端调用。
2. **人类查看入口**：内置本地 HTTP Server，提供 `http://127.0.0.1:NNNN/<secure-token>/` Web 看板，用系统浏览器查看 Orb 内部的知识资产、工具状态、检索效果和运行指标。

本计划将原先的“每个 Orb 内置 Tauri GUI”调整为“每个 Orb 内置 Local Web UI”。Tauri 仍可用于后续的 `mcporb-wizard-gui` 铸造向导，但不进入每个生成 Orb 的运行时依赖。

### 关键产品承诺

- **运行时单二进制**：生成的 Orb 不依赖数据库、外部向量库、WebView、后台服务或云端 API。
- **MCP 优先**：`stdout` 在 Stdio 模式下只允许输出 JSON-RPC 消息，日志统一进入 `stderr` 或文件。
- **本地可视化**：GUI 通过内置 HTTP Server + 系统浏览器实现，不绑定 Tauri/WebKitGTK/WebView2 等桌面运行时。
- **构建期做重活**：PDF 解析、切片、索引、embedding 生成尽量在构建期完成；运行期只加载嵌入式数据并执行查询。

## 2. MVP 测试素材：mda-orb PDF

首个测试素材使用当前仓库中的 PDF：

```text
mda-orb/00-2_MDA_Guide_v1.0.1.pdf
```

该文件约 323KB，文本可抽取，目录和正文结构清晰，适合作为 PDF-first MVP 样本。

### PDF 复杂度边界

PDF 可以进入 MVP，但必须把边界定义清楚：

- MVP 支持 **可文本抽取的 PDF**。
- MVP 不支持扫描版 PDF OCR。
- 若 PDF 抽取后文本密度过低，应给出清晰错误提示：`This PDF appears to be scanned or image-only; OCR is not supported yet.`
- PDF 解析是构建期能力，不进入生成 Orb 的运行时依赖。

### 文档导入策略

`mcporb-core` 中设计统一的文档导入接口：

```text
DocumentImporter
├── MarkdownImporter   # MVP 同步实现，作为管线对照基线
├── PdfImporter        # MVP 同步实现，本计划的核心样本
└── FutureImporters...
```

MVP 同时实现 `MarkdownImporter` 与 `PdfImporter`，**Markdown 先跑通整条索引管线，再把 PDF 接进来**——Markdown 比 PDF 简单一个数量级，先跑通可以避免 PDF 抽取问题污染上层逻辑。

PDF 抽取的具体技术选型：

1. **默认实现**：使用 `pdf-extract` crate（纯 Rust、API 简单、可抽取页码级文本）。
2. **Fallback / 对照**：`lopdf` 提供更底层的解析能力，留作 `pdf-extract` 抽取失败时的二次尝试和单元测试对照。
3. **明确排除**：不使用 `pdftotext`、`pdfium-render` 等需要系统命令或外部动态库的方案。
4. 抽取结果统一转换为内部 `Document`/`Section`/`Chunk` 模型，所有 importer 走同一套索引管线。

## 3. 总体项目拓扑结构

项目采用 Rust Cargo Workspace：

```text
MCPOrb/
├── Cargo.toml
├── README.md
├── dev-plan.md
├── mda-orb/
│   └── 00-2_MDA_Guide_v1.0.1.pdf
├── crates/
│   ├── mcporb-core/             # 文档导入、切片、索引、manifest、构建资产生成
│   ├── mcporb-cli/              # 命令行构建、检查、调试工具
│   └── mcporb-runtime-tpl/      # 每个生成 Orb 的 Rust 运行时模板
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs          # 启动模式判定与任务编排
│           ├── mcp_handler.rs   # Stdio MCP JSON-RPC 服务
│           ├── web_server.rs    # Local HTTP Server 与 API
│           ├── search.rs        # 嵌入式检索运行时
│           └── assets.rs        # include_bytes!/embed 静态资产与索引
└── mcporb-wizard-gui/           # 后续：Tauri 桌面铸造向导，调用 core/cli 能力
```

## 3.1 Orb Binary Generation Mechanism

### Chosen Approach: Option A (v0.1 scope) — `cargo build` subprocess

The CLI generates a temporary copy of `mcporb-runtime-tpl` with all data assets baked in via `include_bytes!` / `rust-embed`, then invokes `cargo build --release` on that workspace as a subprocess. The resulting binary is copied to `<name>.orb` and the temp workspace is cleaned up.

**Build flow:**

```text
mcporb build <source>
  → parse & index source documents (mcporb-core)
  → generate temp workspace (copy of mcporb-runtime-tpl + data assets)
  → cargo build --release  (subprocess)
  → copy ./target/release/<name> → ./<name>.orb
  → clean temp workspace
```

**v0.1 known limitation**: This approach requires the user to have a Rust toolchain (`rustup` / `cargo`) installed on their machine. This is acceptable for v0.1 developer-facing usage but is not suitable for general end-user distribution.

**Upgrade path (v0.3+)**: Ship pre-compiled runtime stubs for each target triple — macOS arm64, macOS x86_64, Linux x86_64, Windows x86_64 — so that `mcporb build` only needs to patch the data section of a pre-built binary, eliminating the Rust toolchain requirement for end users entirely.

### Phase 1 Spike Task

> **[Spike] Validate runtime binary size baseline**: Compile a minimal `mcporb-runtime-tpl` with `rust-embed` + `axum` + `tokio` (full features) as dependencies (no actual business logic). Measure the stripped release binary size (`cargo build --release` + `strip`). Target: ≤ 15 MB. Record the result in this document before proceeding with any feature implementation. If the baseline exceeds 15 MB, escalate immediately and reconsider the dependency set.

> **Spike result (recorded 2026-05-17):** `mcporb-size-spike` stripped release binary = 0.8 MB (801 KB). Budget: ≤ 15 MB. Status: ✅ PASS. Dependencies included: `axum`, `tokio` (full), `rust-embed`, `serde`, `serde_json`, `postcard`, `anyhow`, `tracing`, `tracing-subscriber`, `rand`. Note: `rmcp` was not included in this spike — it is not needed for the HTTP/async/serialization stack measurement and was excluded to avoid dependency resolution issues at this stage.

## 4. Runtime 启动模式设计

生成的 Orb 必须支持明确的命令行参数：

```text
orb                  # 自动模式；默认目标是 stdio + web gui
orb --stdio-gui      # 强制 MCP Stdio + Local Web UI
orb --gui-only       # 只启动 Local Web UI，不读取 stdin
orb --stdio-only     # 只启动 MCP Stdio，不启动 HTTP Server
orb --open           # 启动后自动打开浏览器
orb --no-open        # 禁止自动打开浏览器
orb --port <NNNN>    # 指定本地 HTTP 端口；默认自动选择空闲端口
```

### 自动检测规则

启动模式优先级：

```text
显式命令行参数 > stdin TTY 检测 > 父进程启发式 > 默认策略
```

推荐行为矩阵：

```text
场景                              行为
双击或终端直接运行                 gui-only + auto-open
MCP 客户端通过 stdio 启动           stdio-gui + no-open
开发者显式 --stdio-gui --open       stdio-gui + auto-open
开发者显式 --stdio-only             stdio-only + no-open
开发者显式 --gui-only               gui-only + auto-open
```

Rust 侧核心判断：

```rust
use std::io::IsTerminal;

let stdin_is_terminal = std::io::stdin().is_terminal();
```

父进程名称只能作为辅助信号，**不可作为唯一依据**。已知 MCP 客户端可执行名（小写匹配，子串包含即可）：

```text
claude            # Claude Desktop (macOS/Windows)
claude-desktop
cursor            # Cursor
code              # VS Code / VSCodium（注意需配合 stdin 非 TTY 判定，避免误伤普通终端）
windsurf
zed
```

匹配命中只用于提高 `stdio` 判定的置信度；最终决策仍以 `stdin TTY` + 显式参数为主。

### Stdio 纯净性红线

在任何启用 MCP Stdio 的模式下：

- `stdout` 只能写 JSON-RPC 响应或通知。
- HTTP server 日志不得进入 `stdout`。
- 检索日志、错误、panic hook 输出进入 `stderr` 或日志文件。
- 自动测试必须覆盖 stdout 污染检查。

### Web UI URL Discoverability for MCP-Launched Orbs

当 Orb 由 MCP 客户端启动（无 TTY、无 `--open`）时，用户需要一种方式获取 Web UI 地址：

- 绑定的地址和 token 写入 **stderr**（由 MCP 客户端日志捕获）。
- 同时写入状态文件：`$TMPDIR/mcporb/<orb-name>.url`，内容为完整 URL（含 token）。
- 用户可通过 `mcporb status <orb-name>`（v0.2+）查询，或直接读取该文件。
- 状态文件在进程退出时删除（token 同步作废）。

## 5. Local Web UI 架构

每个 Orb 运行时内置 HTTP Server，仅监听 loopback：

```text
http://127.0.0.1:<dynamic-port>/<secure-token>/
```

安全策略（防 DNS rebinding、防本地嗅探）：

- 默认监听 `127.0.0.1`，不监听 `0.0.0.0`、不绑定外网接口。
- 每次进程启动重新生成 token path，长度 **≥ 128 bit**（建议 32 字节 URL-safe base64，由 `rand::rngs::OsRng` 生成）。
- 所有 `/api/*` 请求必须前缀匹配 token，否则 `404`（不返回 401，避免信息泄漏）。
- 必须校验 `Host` 头：仅接受 `127.0.0.1:<port>` 或 `localhost:<port>`，其他一律 `403`，防 DNS rebinding。
- 进程退出时 token 即作废，不持久化到磁盘。
- Web 页面不需要登录，URL token 是唯一的本地访问凭据。

固定 Rust 依赖（钉死技术栈，避免后续摇摆）：

```text
# 运行时核心
tokio              # 并发运行 MCP loop 与 HTTP server
rmcp               # 官方 Rust MCP SDK，提供 JSON-RPC + 协议类型
axum               # Local HTTP API
tower-http         # 静态资源中间件（trace/cors 按需启用）
rust-embed         # 嵌入 web_assets 目录（选定，弃用 include_dir）
serde + serde_json # manifest JSON 解析
postcard           # chunks/index 二进制序列化（弃用 rmp-serde）
rand               # token 生成
webbrowser         # gui-only / --open 时打开系统浏览器
tracing            # 结构化日志，统一输出到 stderr
anyhow + thiserror # 错误处理：anyhow 用于二进制路径，thiserror 用于库 crate
```

**前端技术栈**（MVP 阶段刻意保持极简）：

- 单文件 `index.html` + 原生 JavaScript（ES modules）。
- **Tailwind CSS 是构建期开发依赖，不是运行时依赖**：
  - Orb 的最终用户和 Orb 消费者**不需要**安装任何 Node.js 工具链。
  - 贡献者（修改 Web UI 模板的人）需一次性安装：`npm install -g tailwindcss`。
  - 仓库中提交一份预生成的 `web_assets/tailwind.css`，仅修改 Rust 代码的贡献者**无需**运行 Tailwind。
  - 仅当 HTML/模板文件变更时才需重新生成：`tailwindcss -i ./src/input.css -o ./web_assets/tailwind.css --minify`。
  - "零 Node 工具链"承诺适用于**最终用户和 Orb 消费者**，不适用于修改 Web UI 的贡献者。
- 不引入 Vue / React / Vite 等任何需要 Node 构建链的方案，直到 v0.3 再评估。
- 整个前端通过 `rust-embed` 编译进二进制。

Web API 初版：

```text
GET  /api/manifest       # Orb 元数据、文档、工具、索引信息
GET  /api/documents      # 文档列表
GET  /api/documents/:id  # 文档抽取文本或章节
POST /api/search         # 调用本地混合检索
GET  /api/metrics        # 请求计数、检索耗时、运行模式
GET  /api/events         # SSE：MCP 请求计数、搜索事件、状态变化
```

前端看板初版页面：

- Orb 概览：名称、描述、版本、构建时间。
- 文档资产：PDF 文件、章节、页码/位置、chunk 分布。
- MCP 能力：resources、tools、schemas。
- 检索调试：输入 query，展示 top-k chunk、score、来源位置。
- 运行指标：MCP 请求数、检索次数、平均耗时、当前启动模式。

## 6. Orb Manifest 与嵌入式资产

序列化格式分两层，避免混用：

- **Manifest = JSON**：人类可读，方便 `mcporb inspect` 直接打印、方便排错。
- **Chunks / 索引 = postcard**：紧凑二进制、纯 Rust、零拷贝友好；弃用 msgpack。

Manifest 字段（`orb_manifest.json`）：

```text
name
version
description
orb_format_version       # 整个 Orb 资产布局的版本
mcp_protocol_version
build_time
source_documents
resources
tools
chunk_count
index_format_version     # BM25 索引的内部格式版本
binary_size_target_mb    # 当前构建目标大小，用于回归监控
permissions
```

构建期生成的资产：

```text
orb_manifest.json
documents.postcard
chunks.postcard
bm25_index.postcard
web_assets/              # 包含 index.html、bundle.js、tailwind.css
# MVP 不生成 vector_index；embedding 推迟到 v0.2 再评估
```

### 索引格式契约（Phase 1 必须先钉死）

`mcporb-core`（writer）与 `mcporb-runtime-tpl`（reader）之间通过下列 Rust struct 形成契约，定义在 `mcporb-core` 的 `format` 模块并被 runtime 直接复用：

```text
struct OrbManifest { ... }            # serde JSON
struct Document { id, title, source_path, page_count, sections }
struct Section   { id, document_id, title, page_start, page_end }
struct Chunk     { id, document_id, section_id, page, span, text }
struct Bm25Index { vocab, postings, doc_lengths, avg_doc_len, params }
```

任何字段变更都必须同步 `orb_format_version` / `index_format_version`，否则 runtime 拒绝加载。

### 资产嵌入策略

- 运行时模板尽量固定，不通过大量字符串替换改 Rust 源码；`mcporb-core` 只生成数据资产，模板用 `include_bytes!` + `rust-embed` 加载。
- **嵌入大小阈值**：单个资产 ≤ 50MB 走 `include_bytes!`；超过阈值时改为运行期释放到 `$TMPDIR` 并 mmap 读取（避免 `include_bytes!` 拖垮编译内存和增量构建）。MDA PDF 样本远低于阈值，MVP 不会触发该路径，但接口需预留。

### 二进制大小预算

"单二进制"是核心卖点，设硬预算并在 CI 监控：

```text
v0.1 (MVP, MDA PDF only)   release 二进制 ≤ 15 MB
v0.2 (Markdown + 看板)     release 二进制 ≤ 20 MB
引入 embedding 时           需单独评估并更新预算
```

release profile 必须开启：`lto = "fat"`、`codegen-units = 1`、`strip = true`、`panic = "abort"`。

## 7. 核心阶段开发计划

### 阶段一：Workspace、CLI、Markdown 与 PDF 导入 MVP

目标：先用合成 Markdown 跑通整条索引管线，再把 `mda-orb/00-2_MDA_Guide_v1.0.1.pdf` 接进同一管线，产出可检索资产。

开发任务：

0. **[Spike] 验证二进制大小基线**：在实现任何功能之前，先编译一个仅包含 `tokio`（full）、`axum`、`rmcp`、`rust-embed`、`serde`、`postcard`、`anyhow`、`tracing` 作为依赖的最小 Rust 二进制（无实际业务逻辑）。执行 `cargo build --release` + `strip`，测量 stripped release 二进制大小。目标：≤ 12 MB。将结果记录在本文档 §3.1 的 Phase 1 Spike Task 节中。若超过 12 MB，立即上报并在继续开发前重新审视依赖集。

1. 建立 Cargo Workspace：`mcporb-core`、`mcporb-cli`、`mcporb-runtime-tpl`。
2. 在 `mcporb-core` 的 `format` 模块定义索引契约 struct（见第 6 章），作为 core 与 runtime 之间的唯一接口。
3. 实现 `DocumentImporter` trait 和 `MarkdownImporter`（按 `#`/`##` 切 section），用 `tests/fixtures/synthetic.md` 跑通整条管线作为对照基线。
4. 实现 `PdfImporter`（基于 `pdf-extract`），抽取 MDA PDF 文本并保留页码与位置线索。
5. 实现基础 chunking：按标题/页/段落优先切片，记录 source span；默认 chunk 大小 800 字符、overlap 100 字符（可在 manifest 配置）。
6. 生成 manifest（JSON）、documents、chunks 的序列化资产（postcard）。
7. CLI 支持：

```bash
mcporb build ./mda-orb --name mda-guide          # 输出到 ./target/orbs/mda-guide/
mcporb build ./docs --name my-docs --format markdown
mcporb inspect ./target/orbs/mda-guide
```

验收标准：

- 合成 Markdown 与 MDA PDF 都能走通同一索引管线。
- 能从 MDA PDF 抽取可读文本；扫描版 PDF 返回明确错误。
- 能生成 chunk 列表和 manifest，且 chunk 反序列化往返一致（核心契约测试）。
- 能在 CLI 中打印文档数、chunk 数、前几个章节/切片。

### 阶段二：Runtime Stdio MCP + Local Web UI 骨架

目标：生成的 Orb 能同时服务 AI 和人类看板。

开发任务：

1. 实现 runtime 启动模式：`--gui-only`、`--stdio-only`、`--stdio-gui`、`--open`、`--no-open`。
2. 实现 stdin TTY 自动检测与父进程启发式检测（识别名单见第 4 章）。
3. 基于 `rmcp` crate 实现 MCP 基础协议：`initialize`、`tools/list`、`tools/call`、`resources/list`、`resources/read`；不自己撸 JSON-RPC 编解码。
4. 实现 Local HTTP Server（axum），提供 manifest/documents/search/metrics API，token 与 Host 头校验按第 5 章实现。
5. 嵌入最小 Web UI（单文件 HTML + 原生 JS + Tailwind），能查看 MDA PDF 资产和运行模式。
6. 通过 SSE 或内部状态计数，让 Web UI 显示 MCP 请求计数。
7. `tracing` 日志统一输出到 stderr；在 stdio 模式下用编译期断言/单元测试守住 stdout 纯净性。

验收标准：

- `orb --gui-only` 打开浏览器看板。
- MCP 客户端 stdio 启动时不自动弹浏览器。
- `stdout` 无任何非 JSON-RPC 输出（CI 测试覆盖）。
- 错误的 token path 返回 404；错误的 Host 头返回 403。
- Web UI 可以查看 MDA PDF 的文档和 chunk 信息。

### 阶段三：本地检索能力

目标：Orb 对 MDA PDF 提供可用的 `search_knowledge` MCP 工具。

开发任务：

1. **自实现轻量 BM25**：构建期生成倒排索引（postcard 序列化），运行期 O(查询词数 × 后置列表)，预计实现量 ~200 行。不引入 `tantivy`（体积太大，破坏单二进制目标）。
2. 中文分词 MVP 阶段使用 unicode segmentation（`unicode-segmentation` crate）+ 简单标点拆分；中文专用分词推迟到 v0.2。
3. **MVP 明确不做 embedding**：不引入 `fastembed-rs` / `candle` / ONNX，否则二进制会膨胀到 80MB+。embedding 推迟到 v0.2 单独立项评估。
4. 将检索能力映射为 MCP `tools/call search_knowledge`。
5. Web UI 检索调试页展示 top-k、score、来源文档、页码/位置、chunk 文本。
6. 建立 MDA PDF 的固定测试 query 集（在 `tests/fixtures/mda_queries.toml`），作为检索回归测试。

验收标准：

- `search_knowledge` 能返回 MDA Guide 中相关片段。
- Web UI 和 MCP 工具调用共用同一套 search runtime。
- 检索延迟、命中结果、错误情况有基本测试覆盖。
- release 二进制大小仍 ≤ 15MB。
- **BM25 搜索延迟 SLA**：在 2020 MacBook Air M1 上，针对最多 10,000 个 chunk 的语料库，查询响应时间必须 < 100ms（p99）。此指标须通过 `cargo bench` 基准测试验证，阶段三完成前必须通过该基准。

> **CJK 分词风险**：默认 BM25 分词器使用 `unicode-segmentation` 词边界，对 CJK 文本的召回率较差。v0.1 可接受，因为主要测试文档（MDA Guide）为英文。CJK 感知分词器（如字符 n-gram fallback）作为 v0.3 增强项跟踪。若源文档包含 CJK 内容，贡献者应在 issue tracker 中标记。

### 阶段四：生成器稳定化与 Wizard GUI

目标：把 CLI 构建能力产品化，并准备面向普通用户的一键铸造体验。

开发任务：

1. 固化 runtime template，不再靠大规模源码替换生成 Orb。
2. 完善 `mcporb-cli build`、`inspect`、`run`、`test-query`。
3. 完善 MarkdownImporter（在阶段一已建立基线），补全列表/代码块/表格的 chunking 处理。
4. 建立 `mcporb-wizard-gui`，使用 Tauri 作为铸造工具界面。
5. Wizard 调用 core/cli 能力，不重复实现构建逻辑。
6. release profile 配置在第 6 章已固化；本阶段补充前端资源最小化（Tailwind purge、JS minify）。
7. 加入二进制大小回归 CI：超过预算（v0.2 ≤ 20MB）则构建失败。

验收标准：

- 普通用户可选择 PDF 文件夹，生成一个 Orb。
- 生成的 Orb 可被 MCP 客户端配置使用。
- 生成的 Orb 可打开本地 Web 看板检查内容。
- 二进制大小符合 v0.2 预算。

## 8. 后续能力路线

```text
v0.1  Markdown + PDF Orb MVP
      - MarkdownImporter (基线)
      - PdfImporter (pdf-extract，MDA PDF)
      - CLI build/inspect
      - Stdio MCP (基于 rmcp)
      - Local Web UI (vanilla JS + Tailwind)
      - search_knowledge (自实现 BM25)
      - 明确不含 embedding / 向量检索

v0.2  Markdown 完善 + embedding 评估
      - Markdown 列表/代码块/表格 chunking
      - 中文分词改进
      - embedding 单独立项评估（fastembed-rs / candle / ONNX 选型 + 二进制大小预算重审）
      - 更丰富 metrics 与检索调试体验

v0.3  OpenAPI -> MCP tools
      - OpenAPI schema 导入
      - tool schema 生成
      - HTTP 调用安全策略
      - 评估是否升级前端到 Vite + 组件框架

v0.4  本地脚本工具
      - Python/Bash tool manifest
      - 权限、超时、工作目录、环境变量策略
      - 沙箱/确认机制

v0.5  分发与可信链路
      - 签名、checksum
      - Orb inspect/verify
      - 跨平台构建矩阵
```

## 9. 关键测试清单

必须优先建立以下测试：

- PDF 抽取：MDA PDF 可抽取文本，低文本密度 PDF 给出明确错误。
- Chunking：chunk 保留文档来源、页码或位置。
- Manifest：版本、文档、工具、索引信息完整。
- MCP 协议：`initialize`、`tools/list`、`tools/call`、`resources/list`、`resources/read`。
- Stdio 纯净性：stdout 只包含 JSON-RPC。
- 启动模式：`--gui-only`、`--stdio-only`、`--stdio-gui`、自动检测。
- Web 安全：仅监听 loopback，token path/API 校验有效。
- 检索：固定 MDA query 返回合理 top-k。
- **BM25 延迟基准**（`cargo bench`）：10,000 chunk 语料库 p99 < 100ms（阶段三完成前必须通过）。
- **CJK 分词回归**：若测试语料包含 CJK 内容，须有专项召回率测试并在 issue tracker 标记。
- 跨平台：macOS 优先，随后 Windows/Linux。

## 10. 当前推荐结论

为了达到 MCPOrb 的总体目标，当前推荐路线是：

1. **生成 Orb 的 runtime 不使用 Tauri，改用内置 HTTP Server + 系统浏览器。**
2. **Tauri 只保留给后续 Wizard GUI 铸造工具。**
3. **MVP 同时支持 Markdown 与 PDF，但先用合成 Markdown 跑通管线，再接 MDA PDF。**
4. **PDF 能力作为统一文档导入管线的一个 importer，而不是一次性特殊逻辑。**
5. **先做 CLI，再做 Wizard。**
6. **始终把 Stdio stdout 纯净性作为最高优先级工程红线。**

### rmcp 稳定性风险

> **rmcp stability risk**: `rmcp` 是官方 Rust MCP SDK，但 MCP 规范仍在演进。缓解措施：钉死到精确版本；在 `mcporb-core` 中将所有 MCP 协议调用封装在一个薄的 `McpAdapter` trait 后面，使 MCP 层可以在不触及业务逻辑的情况下替换。若 `rmcp` 停止维护，备选方案是手写 JSON-RPC 2.0 层（约 500 行）。

## 11. 技术选型锁定（开工前确认）

下列选型在 MVP 阶段视为已锁定，避免后续摇摆：

| 维度 | 选型 | 备注 |
| --- | --- | --- |
| PDF 抽取 | `pdf-extract`（默认）+ `lopdf`（fallback/对照） | 纯 Rust，不依赖系统命令 |
| MCP 协议 | `rmcp` 官方 Rust SDK | 不自己撸 JSON-RPC |
| HTTP Server | `axum` + `tower-http` | |
| 异步运行时 | `tokio` | |
| 检索 | 自实现 BM25 ~200 行 | 不引入 tantivy |
| 中文分词 | `unicode-segmentation` MVP | 专用分词推迟到 v0.2 |
| Embedding | **MVP 不做** | v0.2 单独评估 |
| Manifest 格式 | JSON (`serde_json`) | 人类可读 |
| 索引 / chunks 格式 | postcard | 紧凑、纯 Rust |
| 资产嵌入 | `rust-embed` + `include_bytes!`（≤50MB 阈值） | |
| 前端 | 单文件 HTML + 原生 JS + Tailwind | 零 Node 工具链（最终用户）；贡献者需 tailwindcss CLI |
| 日志 | `tracing` → stderr | stdout 受 stdio 红线保护 |
| 错误处理 | `anyhow`（bin）+ `thiserror`（lib） | |
| Token | 32 字节 OsRng base64，每次启动重生成 | + Host 头校验 |
| 二进制预算 | v0.1 ≤ 15MB，v0.2 ≤ 20MB | CI 回归 |

任何对上表的调整都应当作为正式决定写回本计划，再更新代码。

### 依赖版本基线（Pinned Baseline Versions）

所有版本在 `[workspace.dependencies]` 中声明一次，成员 crate 通过 `workspace = true` 继承，不在各 crate 中重复指定版本。

```toml
rmcp = "0.1"                                        # 关注 breaking changes；稳定后钉死到精确版本
axum = "0.8"
tokio = { version = "1", features = ["full"] }
rust-embed = "8"
serde = { version = "1", features = ["derive"] }
postcard = { version = "1", features = ["alloc"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
webbrowser = "1"
rand = "0.8"
```

> **注意**：`rmcp` 目前处于早期阶段，需持续关注上游 breaking changes。一旦 API 稳定，应将版本锁定为精确版本（如 `rmcp = "=0.1.x"`）。参见 §10 rmcp 稳定性风险说明。
