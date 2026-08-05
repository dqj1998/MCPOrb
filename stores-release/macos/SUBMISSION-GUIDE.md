# MCPOrb Runner — Mac App Store 提交指南

## 当前状态

| 组件 | 状态 | 说明 |
|------|------|------|
| App 图标 | ✅ 已就绪 | `icons/icon_1024x1024.png` |
| 截图 | ✅ 已就绪 | 4 种尺寸 × 4 个场景 |
| 描述/关键词/URL | ✅ 已就绪 | `metadata/` 目录 |
| Apple Distribution 证书 | ❌ 缺失 | 需要在 Apple Developer Portal 创建 |
| MAS Provisioning Profile | ❌ 缺失 | 需要创建 MCPOrb Runner 专用配置 |
| App Store Connect 记录 | ❌ 未知 | 需要在 App Store Connect 创建应用 |

## 审核被拒时的回复

若收到 Guideline 2.4.5(i) 类审核拒绝,先阅读
[`REVIEW-REPLY-2.4.5.md`](REVIEW-REPLY-2.4.5.md):

- **权限疑问类**(如 `com.apple.security.network.server`):内有可直接粘贴的英文回复模板,通常无需重新提交。
- **容器内保存用户文件类**:需要代码修复后重新提交(库文件夹 + security-scoped bookmark 方案,仅 macOS 生效,不影响 Windows)。

## 提交步骤

### 第一步：Apple Developer Portal 准备

#### 1.1 创建 Apple Distribution 证书

1. 登录 https://developer.apple.com/account/resources/certificates
2. 点击 "+" 创建新证书
3. 选择 **Apple Distribution** (不是 Apple Development)
4. 上传 CSR (Certificate Signing Request)
   - 打开 **钥匙串访问** → **钥匙串访问** → **证书助理** → **从证书颁发机构请求证书**
   - 填写邮箱，选择"存储到磁盘"
5. 下载并双击安装证书

#### 1.2 创建 App ID

1. 登录 https://developer.apple.com/account/resources/identifiers
2. 点击 "+" 创建新 Identifier
3. 选择 **App IDs** → **App**
4. 填写：
   - Description: `MCPOrb Runner`
   - Bundle ID: `com.mcporb.runner` (必须与 tauri.conf.json 一致)
5. 启用以下 Capabilities:
   - ✅ App Sandbox
   - ✅ Network Connections (Client)
   - ✅ Network Connections (Server)
   - ✅ Files → User Selected Files (Read/Write)

#### 1.3 创建 Mac App Store Provisioning Profile

1. 登录 https://developer.apple.com/account/resources/profiles
2. 点击 "+" 创建新 Profile
3. 选择 **Mac App Store** → **macOS**
4. 选择 App ID: `com.mcporb.runner`
5. 选择 Apple Distribution 证书
6. 命名: `MCPOrb Runner MAS`
7. 下载并双击安装

### 第二步：App Store Connect

#### 2.1 创建应用记录

1. 登录 https://appstoreconnect.apple.com
2. 点击 **我的 App** → 点击 "+" → **新建 App**
3. 填写信息:
   - 平台: ✅ macOS
   - 名称: `MCPOrb Runner`
   - 主要语言: English
   - Bundle ID: `com.mcporb.runner`
   - SKU: `mcporb-runner-macos`
   - 用户访问权限: 完全访问权限

#### 2.2 填写应用信息

在 App Store Connect 中填写:

**App 信息**:
- 名称: `MCPOrb Runner`
- 副标题: `Orb knowledge bases for MCP`
- 类别: Developer Tools (主要)
- 内容版权: © 2026 MCPOrb

**价格与销售范围**:
- 价格: Free (或设置价格)
- 销售范围: 全球

**App 隐私**:
- 链接: https://mcporb.ai/privacy
- 数据收集: 不收集用户数据

**版本信息**:
- 版本: 1.1.9
- 版权: © 2026 MCPOrb
- 营销 URL: https://mcporb.ai
- 支持 URL: https://mcporb.ai/support

**描述**:
```
MCPOrb Runner imports Orb ZIP knowledge bases, provides local semantic search, and generates MCP client configuration for Claude Desktop, Cursor, and VS Code.

Orbs are portable, self-contained MCP knowledge packs — one file contains search index, runtime, and structured knowledge. No npm install, no Docker, no cloud dependency.

Key features:
• Install and manage Orb ZIPs with one click
• Multi-strategy local search (BM25, TF-IDF, Trigram, Vector, Hybrid)
• Auto-generate MCP STDIO config for Claude Desktop, Cursor, VS Code
• Built-in HTTP MCP server for Streamable HTTP transport
• Deep link support for mcporb:// URLs
• macOS native sandbox with user-selected file access
```

**关键词**:
```
MCP,Model Context Protocol,Claude Desktop,Cursor,VS Code,AI,LLM,knowledge base,Orb,search,RAG,local AI,developer tools,Anthropic,RAG retrieval,semantic search,BM25,OpenAI,AI assistant
```

#### 2.3 上传截图

上传 `screenshots/` 目录中的截图 (选择 2560x1600 或 2880x1800 版本):
- Library tab
- Config tab
- HTTP tab
- Settings tab

#### 2.4 上传应用图标

上传 `icons/icon_1024x1024.png`

### 第三步：构建应用

#### 3.1 确保证书和 Profile 已安装

```bash
# 验证证书
security find-identity -v -p basic | grep "Apple Distribution"

# 验证 Provisioning Profile
ls -la ~/Library/MobileDevice/Provisioning\ Profiles/ | grep mcporb
```

#### 3.2 构建 MAS 版本（推荐脚本）

```bash
cd /Users/dqj/HDD/MCPOrbPrjs/MCPOrb

# 一键构建 + 签名 + 打包（会自动校验 sandbox entitlement）
scripts/build-mas.sh 1.1.9
```

#### 3.3 手动签名应用（仅在调试脚本时使用）

```bash
APP_PATH=$(find target -name "MCPOrb Runner.app" -type d | head -1)
DIST_IDENTITY="Apple Distribution: Qingjie Du (RQQ6N82NA8)"

# mcporb-runtime: MAS 包内也需要带 sandbox entitlement
codesign --force --sign "$DIST_IDENTITY" \
  --entitlements crates/mcporb-runtime-app/entitlements-mas.plist \
  --options runtime --timestamp \
  "$APP_PATH/Contents/MacOS/mcporb-runtime"

# mcporb-gateway-stdio: 同样需要 sandbox entitlement（Transporter 会拒绝未沙盒化的可执行文件）
codesign --force --sign "$DIST_IDENTITY" \
  --entitlements crates/mcporb-runtime-app/entitlements-mas.plist \
  --options runtime --timestamp \
  "$APP_PATH/Contents/MacOS/mcporb-gateway-stdio"

# mcporb-runner: Tauri GUI 应用，需要沙盒权限
codesign --force --sign "$DIST_IDENTITY" \
  --entitlements crates/mcporb-runtime-app/entitlements-mas.plist \
  --options runtime --timestamp \
  "$APP_PATH/Contents/MacOS/mcporb-runner"

# 签名 app bundle
codesign --force --sign "$DIST_IDENTITY" \
  --entitlements crates/mcporb-runtime-app/entitlements-mas.plist \
  --timestamp \
  "$APP_PATH"

# 校验 mcporb-runner 是否已带 sandbox entitlement
codesign -d --entitlements :- "$APP_PATH/Contents/MacOS/mcporb-runner" 2>&1 | grep -A1 "com.apple.security.app-sandbox"
```

**注意**: 用于 Mac App Store 上传的 `.pkg` 中，`MCPOrb Runner.app/Contents/MacOS/` 下的可执行文件都必须包含 `com.apple.security.app-sandbox=true`。

#### 3.4 创建安装包

```bash
# 由脚本生成：MCPOrbRunner-1.1.9-mas.pkg
# 如果手动打包，使用 productbuild（不要用 pkgbuild --root "$APP_PATH"）
productbuild \
  --component "$APP_PATH" /Applications \
  --sign "3rd Party Mac Developer Installer: Qingjie Du (RQQ6N82NA8)" \
  --timestamp \
  MCPOrbRunner-1.1.9-mas.pkg
```

### 第四步：上传到 App Store Connect

#### 方法 A: 使用 Transporter (推荐)

1. 从 Mac App Store 安装 [Transporter](https://apps.apple.com/app/transporter/id1450874784)
2. 打开 Transporter
3. 登录你的 Apple ID
4. 拖拽 `.pkg` 文件到 Transporter
5. 点击"交付"

#### 方法 B: 使用 xcrun notarytool

```bash
# 上传 (需要 App Store Connect API Key)
xcrun notarytool submit MCPOrbRunner-1.1.9-mas.pkg \
  --key ~/path/to/AuthKey_XXXXXXXXXX.p8 \
  --key-id XXXXXXXXXX \
  --issuer XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX \
  --wait
```

### 第五步：提交审核

1. 在 App Store Connect 中，进入 **App Store** → **iOS App / macOS App**
2. 选择版本 1.1.9
3. 在 **构建** 部分选择刚刚上传的构建
4. 填写审核信息:
   - 联系人信息
   - 测试账号 (如果有)
   - 审核备注
5. 点击 **提交以供审核**

## 常见问题

### Q: 签名时提示 "No identity found for signing"
A: 确保 Apple Distribution 证书已安装且在钥匙串中可见。

### Q: 构建失败 "No such module 'Tauri'"
A: 确保已安装所有依赖: `cargo tauri build` 会自动处理。

### Q: App Store Connect 找不到构建
A: 确保使用正确的 Bundle ID (`com.mcporb.runner`) 和版本号。

### Q: 审核被拒 "App Sandbox"
A: 确保 entitlements-mas.plist 包含所有必需的权限。

## 文件清单

```
stores-release/macos/
├── SUBMISSION-GUIDE.md          # 本指南
├── icons/
│   └── icon_1024x1024.png       # App Store 图标
├── screenshots/                  # App Store 截图
│   ├── mcporb-runner-mac-conf-*.png
│   ├── mcporb-runner-mac-http-*.png
│   ├── mcporb-runner-mac-lib-*.png
│   └── mcporb-runner-mac-settings-*.png
└── metadata/
    ├── description.txt           # 应用描述
    ├── keywords.txt              # 关键词
    ├── marketing_url.txt         # 营销 URL
    ├── privacy_url.txt           # 隐私政策 URL
    └── support_url.txt           # 支持 URL
```

## 下一步行动

1. ⬜ 创建 Apple Distribution 证书
2. ⬜ 创建 App ID (com.mcporb.runner)
3. ⬜ 创建 MAS Provisioning Profile
4. ⬜ 在 App Store Connect 创建应用记录
5. ⬜ 构建 MAS 版本
6. ⬜ 签名并打包
7. ⬜ 上传到 App Store Connect
8. ⬜ 提交审核
