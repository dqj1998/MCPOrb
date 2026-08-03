# MCPOrb Runner — App Store 审核回复模板

> 用途:审核被拒(Guideline 2.4.5(i),2026-07-14,Submission ad99d394-ada4-42c1-a669-7b31ebed33ac,版本 1.1.10)后,在 App Store Connect 的 "Reply to Review" 中回复。

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

## 问题 2:用户文件保存在容器内(需要代码修复 + 重新提交)

**结论:不能只回复。** 审核员明确要求"save user files to a location selected by or available to users, using standard Save dialogs",纯文字回复大概率再次被拒。

已实现修复(mcporb-runtime-app v1.2.3+,仅 macOS 生效,Windows 行为不变):

1. **Settings → "Orb Library Folder"**(仅 macOS 显示):标准文件夹选择对话框,用户选择如 `~/Documents/MCPOrb` 的可见位置。
2. 导入的 Orb ZIP 存入 `<选定文件夹>/Orbs/`,不再复制进沙盒容器;registry.json / metrics 等应用数据仍留在容器。
3. 选择时创建 **security-scoped bookmark** 持久化到 settings.json,重启后自动恢复访问权限(子进程 `mcporb-runtime` 继承访问,HTTP/STDIO 模式均可读取)。
4. 未配置库文件夹时回退到原容器位置(兼容旧版本用户)。

### 提交新版本时在 "App Review Information" 附注(可选,帮助审核员复现)

```
Steps to reproduce the Orb Library folder feature:
1. Launch MCPOrb Runner.
2. Open the Settings tab → "Orb Library Folder" → "Choose…".
3. Pick a folder such as ~/Documents/MCPOrb.
4. Import an Orb ZIP. The file is stored inside the chosen folder (visible in Finder).
```

### 本地验证清单(重新提交前)

```bash
# 1. 全量测试
cargo test --workspace

# 2. 构建 + 签名 + 打包(版本号自定)
scripts/build-mas.sh 1.2.3

# 3. 安装 pkg 后手动验证
#    - 设置页可见 "Orb Library Folder" 行(仅 macOS)
#    - 选择文件夹 → 导入 ZIP → Finder 中可见该 ZIP
#    - 完全退出 App 后重启 → 导入的 Orb 仍可搜索/启动(bookmark 生效)
#    - HTTP 标签页启动 Orb → 局域网绑定选项仍可用
```

### 相关文件

| 文件 | 改动 |
|---|---|
| `crates/mcporb-runtime-app/src/macos_access.rs` | 新增:CoreFoundation FFI(bookmark 创建/解析/访问守卫,仅 macOS) |
| `crates/mcporb-runtime-app/src/main.rs` | `choose_orb_library_dir` / `get_platform` 命令;启动时恢复 bookmark 访问;`save_settings` 合并库字段 |
| `crates/mcporb-runtime-app-core/src/settings.rs` | `RuntimeSettings` 新增 `orb_library_dir` / `orb_library_bookmark`(serde default,旧配置兼容) |
| `crates/mcporb-runtime-app-core/src/registry.rs` | `RegistryStore::with_orbs_dir`(注册表元数据与 Orbs 目录分离) |
| `crates/mcporb-runtime-app/frontend/{index.html,app.js,style.css}` | Settings 页 Orb Library 文件夹选择(macOS 仅显示,三语 i18n) |
