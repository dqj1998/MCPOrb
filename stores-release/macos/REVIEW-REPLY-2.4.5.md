# MCPOrb Runner — App Store 审核回复模板

> 用途:审核被拒(Guideline 2.4.5(i),版本 1.3.0)后,在 App Store Connect 的 "Reply to Review" 中回复。

## 问题 1:`com.apple.security.network.server` 权限说明(直接回复,无需重新提交)

直接复制以下英文文本到 App Store Connect 回复框:

---

**Reply to Guideline 2.4.5(i) — com.apple.security.network.server**

The `com.apple.security.network.server` entitlement is actively used by core, user-facing functionality of MCPOrb Runner.

MCPOrb Runner installs "Orb" knowledge bases and serves them to AI assistants (Claude Desktop, Cursor, VS Code, etc.) over the Model Context Protocol (MCP). The app bundles a helper executable, `mcporb-runtime`, which implements the MCP "Streamable HTTP" transport. When the user opens the app's "HTTP" tab and starts an Orb (the "Start HTTP" action), the app launches `mcporb-runtime` as a sandboxed child process with a `--port` argument. The runtime then creates a TCP listener and serves the Orb's search/tools endpoints over HTTP to connected MCP clients.

Details:
- The server socket is opened by the bundled `mcporb-runtime` executable, which is code-signed with the same entitlements (including `com.apple.security.network.server`) so it can bind the listening port inside the sandbox.
- By default the server binds to the loopback interface (127.0.0.1, default port 5599) for local MCP clients.
- The Settings tab includes a "Network binding" option. When set to "External", the app passes `--bind-external` to the runtime, which then binds 0.0.0.0 to accept incoming connections from other devices on the user's LAN — this is the inbound-connection functionality the entitlement covers.
- Outbound connections (Store API, metrics) are covered by `com.apple.security.network.client`.

Removing this entitlement would disable the app's advertised HTTP MCP server feature. We confirm it is not vestigial and is required for the app to function as described in its App Store listing.

---

## 问题 2:用户文件保存在容器内(已修复,版本 1.3.0)

**已修复:** Store 下载现在直接保存到用户选择的 Orb Library 文件夹,不再存储在沙盒容器内。

### 修复内容 (v1.3.0)

1. **Store 下载路径修复**: `store_download_artifact` 函数现在直接下载到 Orb Library 文件夹 (`<orb_library_dir>/Orbs/`) 或默认位置 (`~/.mcporb/Orbs/`),而不是沙盒容器内的临时目录。
2. **用户数据完全在容器外**: 所有用户文件 (Orb ZIPs、设置、注册表) 都存储在 `~/.mcporb/` 或用户选择的文件夹中,完全在沙盒容器外。
3. **向后兼容**: 旧版本用户的数据会自动迁移到新位置。

### 提交新版本时在 "App Review Information" 附注

```
Version 1.3.0 changes:
- Store downloads now save directly to the user-selected Orb Library folder (or ~/.mcporb/Orbs/ if not configured)
- All user data (Orb ZIPs, settings, registry) is stored outside the app sandbox container
- No user files are stored in the hidden container directory

Steps to verify:
1. Launch MCPOrb Runner.
2. Open the Settings tab → "Orb Library Folder" → "Choose…".
3. Pick a folder such as ~/Documents/MCPOrb.
4. Download an Orb from the Store. The file is saved directly to the chosen folder.
5. Import an Orb ZIP. The file is stored inside the chosen folder (visible in Finder).
```

### 本地验证清单(重新提交前)

```bash
# 1. 全量测试
cargo test --workspace

# 2. 构建 + 签名 + 打包(版本号自定)
scripts/build-mas.sh 1.3.0

# 3. 安装 pkg 后手动验证
#    - 设置页可见 "Orb Library Folder" 行(仅 macOS)
#    - 选择文件夹 → 从 Store 下载 Orb → Finder 中可见该 ZIP (不在容器内)
#    - 选择文件夹 → 导入本地 ZIP → Finder 中可见该 ZIP
#    - 完全退出 App 后重启 → 导入的 Orb 仍可搜索/启动(bookmark 生效)
#    - HTTP 标签页启动 Orb → 局域网绑定选项仍可用
```

### 相关文件

| 文件 | 改动 |
|---|---|
| `crates/mcporb-runtime-app/src/main.rs` | `store_download_artifact` 下载到 Orb Library 文件夹而非沙盒容器 |
| `crates/mcporb-runtime-app/src/macos_access.rs` | CoreFoundation FFI (bookmark 创建/解析/访问守卫, 仅 macOS) |
| `crates/mcporb-runtime-app-core/src/settings.rs` | `RuntimeSettings` 包含 `orb_library_dir` / `orb_library_bookmark` |
| `crates/mcporb-runtime-app-core/src/registry.rs` | `RegistryStore::with_orbs_dir` (注册表元数据与 Orbs 目录分离) |
| `crates/mcporb-runtime-app/frontend/{index.html,app.js,style.css}` | Settings 页 Orb Library 文件夹选择 (macOS 仅显示, 三语 i18n) |
