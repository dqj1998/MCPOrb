# MCPOrb Runner — Windows Store 提交指南

## 当前状态

| 组件 | 状态 | 说明 |
|------|------|------|
| Store 图标 | ✅ 已就绪 | `icons/` 目录，8 个尺寸 |
| 截图 | ⬜ 待截图 | 需要在 Windows 上运行应用后截图 |
| 描述/关键词/URL | ✅ 已就绪 | `metadata/` 目录 |
| Package.appxmanifest | ✅ 已就绪 | MSIX 打包清单 |
| Privacy Policy | ✅ 已就绪 | `privacy-policy.md` |
| Microsoft 开发者账号 | ❌ 缺失 | 需要在 Microsoft Partner Center 注册 |
| 代码签名证书 | ❌ 缺失 | 需要购买或使用测试证书 |

## 提交步骤

### 第一步：Microsoft Partner Center 准备

#### 1.1 注册开发者账号

1. 登录 https://partner.microsoft.com/dashboard
2. 注册 **Windows Dev Center** 账号
3. 支付一次性注册费 ($19 USD)
4. 完成账户验证

#### 1.2 创建应用记录

1. 在 Partner Center 点击 **应用和游戏** → **+ 新应用**
2. 填写信息:
   - 产品名称: `MCPOrb Runner`
   - 语言: English (美国)
   - 记类型: 应用
   - 平台: Windows
3. 获取 **产品 ID** (如 `9NBLGGH4RZVR`)

### 第二步：准备发布物料

#### 2.1 图标 (已完成)

`icons/` 目录包含所有必需尺寸:
- StoreLogo.png (50x50) — Store 标志
- Square44x44Logo.png (44x44) — 小磁贴
- SmallTile.png (71x71) — 小磁贴
- Square150x150Logo.png (150x150) — 中磁贴
- Wide310x150Logo.png (310x150) — 宽磁贴
- Square310x310Logo.png (310x310) — 大磁贴
- SplashScreen.png (620x300) — 启动画面
- BadgeLogo.png (24x24) — 徽章图标

#### 2.2 截图 (需手动截取)

在 Partner Center 中上传截图:
- 最小分辨率: 1366 x 768
- 推荐分辨率: 1920 x 1080
- 格式: PNG
- 至少 1 张截图，最多 8 张

截取方式:
```powershell
# 1. 构建并运行应用
cargo tauri build -p mcporb-runtime-app
# 或使用构建脚本:
.\crates\mcporb-runtime-app\scripts\build-mas.ps1

# 2. 运行 MCPOrb Runner

# 3. 使用 Windows 截图工具 (Win+Shift+S) 或
#    运行截图脚本:
.\stores-release\windows\screenshots\capture.ps1
```

建议截取的界面:
1. **Library** — 显示已导入的 Orb 知识库
2. **MCP Config** — 显示生成的 STDIO 配置
3. **HTTP** — HTTP MCP 服务器设置
4. **Settings** — 应用设置

#### 2.3 描述 (已填写)

在 Partner Center 填写:

**短描述** (≤ 100 字符):
```
Run Orb ZIP knowledge bases for MCP clients
```

**长描述** (≤ 4000 字符):
```
MCPOrb Runner imports Orb ZIP knowledge bases, provides local semantic search, and generates MCP client configuration for Claude Desktop, Cursor, and VS Code.

Orbs are portable, self-contained MCP knowledge packs — one file contains search index, runtime, and structured knowledge. No npm install, no Docker, no cloud dependency.

Key features:
• Install and manage Orb ZIPs with one click
• Multi-strategy local search (BM25, TF-IDF, Trigram, Vector, Hybrid)
• Auto-generate MCP STDIO config for Claude Desktop, Cursor, VS Code
• Built-in HTTP MCP server for Streamable HTTP transport
• Deep link support for mcporb:// URLs
• Windows native integration with user-selected file access
```

**关键词** (≤ 10 个):
```
MCP, Model Context Protocol, Claude Desktop, Cursor, VS Code, AI, LLM, knowledge base, Orb, search
```

#### 2.4 分类

- 主要类别: **Developer Tools**
- 次要类别: **Utilities**

#### 2.5 链接

- 隐私政策: https://mcporb.ai/privacy
- 支持 URL: https://mcporb.ai/support
- 营销 URL: https://mcporb.ai

#### 2.6 年龄分级

- 评级: Everyone (所有人)
- 内容描述: None (无)

#### 2.7 版权

```
© 2026 MCPOrb
```

### 第三步：构建 MSIX 安装包

#### 3.1 前置条件

- Windows 10/11
- Windows SDK (含 makeappx.exe)
- Rust toolchain

安装 Windows SDK:
```powershell
# 检查 makeappx 是否已安装
Get-Command makeappx.exe -ErrorAction SilentlyContinue

# 如果未安装，从以下链接下载:
# https://developer.microsoft.com/en-us/windows/downloads/windows-sdk/
```

#### 3.2 同步版本号

```powershell
# 确保 Package.appxmanifest 版本与 Cargo.toml 一致
.\stores-release\windows\sync-version.ps1

# 查看变更 (不实际修改)
.\stores-release\windows\sync-version.ps1 -DryRun
```

#### 3.3 构建 MSIX

```powershell
# 完整构建 (包含 cargo build 侧车 + cargo tauri build + MSIX 打包)
.\stores-release\windows\build-msix.ps1

# 仅打包 (跳过全部构建步骤，使用现有 target/release/ 下的二进制文件)
.\stores-release\windows\build-msix.ps1 -SkipBuild

# 仅跳过侧车构建 (已编译好侧车，只重新构建 Tauri 应用)
.\stores-release\windows\build-msix.ps1 -SkipSidecar

# Debug 构建
.\stores-release\windows\build-msix.ps1 -Configuration Debug

# 构建后验证
.\stores-release\windows\verify-msix.ps1
```

构建产物位置: `target\msix\MCPOrbRunner.msix`

#### 3.5 本地测试安装

```powershell
# 安装 MSIX 进行测试
Add-AppxPackage -Path "target\msix\MCPOrbRunner.msix"

# 卸载
Get-AppxPackage *MCPOrb* | Remove-AppxPackage
```

### 第四步：上传到 Partner Center

1. 登录 https://partner.microsoft.com/dashboard
2. 进入 **应用和游戏** → 选择 `MCPOrb Runner`
3. 点击 **应用提交** → **创建新提交**
4. 选择 **应用包** → 上传 `.msix` 文件
5. 填写所有必填字段 (描述、截图、分类等)
6. 点击 **提交审核**

### 第五步：审核与发布

1. Microsoft 审核通常需要 1-3 个工作日
2. 收到审核结果后:
   - 通过: 自动发布到 Store
   - 被拒: 根据反馈修改后重新提交
3. 发布后在 Store 中搜索 "MCPOrb Runner" 验证

## 常见问题

### Q: makeappx.exe 找不到
A: 确保安装了 Windows SDK。安装后重启终端，或手动添加到 PATH。

### Q: MSIX 安装失败 "此应用包的签名与任何现有证书都不匹配"
A: 本地测试需要信任测试证书。使用以下命令信任:
```powershell
# 创建自签名证书
$cert = New-SelfSignedCertificate -CN "CN=MCPOrb" -CertStoreLocation Cert:\LocalMachine\My -Type CodeSigningCert

# 信任证书
certutil -addstore TrustedPeople $cert.Thumbprint
```

### Q: Store 审核被拒 "应用必须在沙盒中运行"
A: 确保 Package.appxmanifest 中的 Capabilities 只包含 `runFullTrust`。

### Q: 应用启动后崩溃
A: 检查:
1. WebView2 Runtime 是否已安装
2. 所有 DLL 是否随 MSIX 打包
3. 在 Event Viewer 中查看错误日志

### Q: Store 搜索找不到应用
A: 新应用发布后可能需要 24-48 小时才会出现在搜索结果中。

## 文件清单

```
stores-release/windows/
├── SUBMISSION-GUIDE.md              # 本指南
├── Package.appxmanifest             # MSIX 打包清单
├── icons/                           # Store 图标 (8 个尺寸)
│   ├── StoreLogo.png               # 50x50
│   ├── Square44x44Logo.png         # 44x44
│   ├── SmallTile.png               # 71x71
│   ├── Square150x150Logo.png       # 150x150
│   ├── Wide310x150Logo.png         # 310x150
│   ├── Square310x310Logo.png       # 310x310
│   ├── SplashScreen.png            # 620x300
│   └── BadgeLogo.png               # 24x24
├── screenshots/                     # Store 截图 (需手动截取)
│   └── README.txt
├── metadata/
│   ├── description.txt              # 应用描述
│   ├── keywords.txt                 # 关键词
│   ├── marketing_url.txt            # 营销 URL
│   ├── privacy_url.txt              # 隐私政策 URL
│   └── support_url.txt              # 支持 URL
├── privacy-policy.md                # 隐私政策 (供参考)
└── README.md                        # 本目录说明
```

## 下一步行动

1. ⬜ 注册 Microsoft Partner Center 开发者账号
2. ⬜ 在 Partner Center 创建应用记录
3. ⬜ 运行应用并截取 4 张截图 (参考 `screenshots/README.txt`)
4. ✅ 运行 `.\stores-release\windows\sync-version.ps1` — 版本已同步 (v1.2.1)
5. ✅ 运行 `.\stores-release\windows\build-msix.ps1` — MSIX 已构建
6. ⬜ 本地测试安装 MSIX:
   ```powershell
   Add-AppxPackage -Path "target\msix\MCPOrbRunner.msix"
   ```
7. ⬜ 上传到 Partner Center
8. ⬜ 提交审核
