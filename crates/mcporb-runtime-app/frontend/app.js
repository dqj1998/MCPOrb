const invoke = window.__TAURI__?.core?.invoke;

const state = {
  orbs: [],
  orbSecurityById: {},
  activeTab: 'library',
  orbSearchTargetId: null,
  orbSearchPassword: null,
  platformConfigs: [],
  runningOrbIds: [],
  qaOrbId: null,
  qaPage: 1,
  qaTotalPages: 1,

  pendingDeleteOrbId: null,
  pendingImportOrbId: null,
  pendingDownloadArtifactId: null,
  pendingStoreArtifactId: null,
  storeSearchState: { query: '', tag: null, method: null, page: 1 },
  storeView: 'browse',
  libraryPage: 1,
  libraryPageSize: 20,
  libraryTotalPages: 1,
  platform: 'unknown',
  orbLibraryDir: null,
  pendingLibraryChange: null,
  pendingLibraryDelete: false,
  // True while the library-change modal was opened from the first-launch
  // onboarding flow; cancelling then returns the user to onboarding.
  fromOnboarding: false,
};

const importState = {
  selectedPath: null,
};

const $ = (id) => document.getElementById(id);

function isMacPlatform(platform) {
  return ['macos', 'darwin', 'osx'].includes(String(platform || '').toLowerCase());
}

// ── Theme (Light / Dark / System) ─────────────────────────────────────────

const THEME_KEY = 'mcporb-runner-theme';

function getSystemTheme() {
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function applyTheme(theme) {
  const html = document.documentElement;
  if (theme === 'system') {
    html.setAttribute('data-theme', getSystemTheme());
  } else {
    html.setAttribute('data-theme', theme);
  }
}

function initTheme() {
  const saved = localStorage.getItem(THEME_KEY);
  const theme = saved || 'system';
  const sel = document.getElementById('theme-select');
  if (sel) sel.value = theme;
  applyTheme(theme);
  window.matchMedia('(prefers-color-scheme: light)').addEventListener('change', () => {
    const current = document.getElementById('theme-select')?.value || 'system';
    if (current === 'system') applyTheme('system');
  });
}

function setTheme(theme) {
  localStorage.setItem(THEME_KEY, theme);
  applyTheme(theme);
}

// ── i18n ──────────────────────────────────────────────────────────────────

const LOCALE_KEY = 'mcporb-runtime-locale';

const locales = {
  en: {
    /* header */
    'app.title': 'MCPOrb Runner',
    'theme.system': 'System',
    'theme.light': 'Light',
    'theme.dark': 'Dark',
    'tab.library': 'Library',
    'tab.mcp': 'MCP Config',
    'tab.store': 'Store',
    'tab.http': 'HTTP',
    'tab.settings': 'Settings',
    /* library */
    'library.title': 'Library',
    'library.import': 'Import',
    'library.refresh': 'Refresh',
    'library.filter_placeholder': 'Search orbs by name or description…',
    'library.filter_btn': 'Filter',
    'library.search_btn': 'Search',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': 'No Orbs installed yet. Click Import to add an Orb ZIP.',
    'library.no_match': 'No Orbs match "{query}".',
    'library.qa_btn': 'Service Requests',
    'library.delete_title': 'Delete Orb',
    'library.delete_btn': 'Delete',
    'library.delete_confirm': 'Are you sure you want to delete "{name}"?',
    'library.delete_success': 'Deleted {name}',
    'library.password_badge': 'Password',
    'library.password_every_launch': 'Every launch',
    'library.password_remembered': 'Remembered',
    'library.restart_hint': 'If MCP clients (Claude, Cursor, etc.) are running, restart them to apply the updated Orb.',
    'library.bookmark_stale_banner': 'Orb library folder access has expired (e.g. after an app update). Orbs are unavailable until you re-select the folder.',
    'library.bookmark_stale_fix_btn': 'Re-select…',
    'library.stats_requests': 'Requests: {total}',
    'library.stats_searches': 'Search: {n}',
    'library.stats_stdio': 'STDIO: {n}',
    'library.stats_http': 'HTTP: {n}',
    'library.page_info': 'Page {page} of {total}',
    /* store */
    'store.title': 'Store',
    'store.search_placeholder': 'Search Orbs in MCP Store',
    'store.search_btn': 'Search',
    'store.detail_title': 'Orb Details',
    'store.detail_back_btn': 'Back',
    'store.version_label': 'Version',
    'store.versions_title': 'Versions',
    'store.artifacts_title': 'Artifacts',
    'store.download_zip_btn': 'Download ZIP',
    'store.password_label': 'Password',
    'store.password_placeholder': 'Enter password',
    'store.password_submit': 'Submit',
    'store.password_required_title': 'Password Required',
    'store.password_required_label': 'Enter password for this artifact:',
    'store.password_verifying': 'Verifying...',
    'store.password_incorrect': 'Incorrect download password. Please try again.',
    'store.password_network_error': 'Could not connect to the store server. Please check your network and try again.',
    'store.download_error': 'Download error: {error}',
    'store.artifact_downloaded': 'Downloaded: {result}',
    'store.tags_filter': 'Tag',
    'store.tag_all': 'All tags',
    'store.method_filter': 'Method',
    'store.method_all': 'All methods',
    'store.no_detail': 'No detail available.',
    'store.artifact_kind_canonical': 'Canonical',
    'store.artifact_kind_derived': 'Derived',
    'store.password_status_all': 'All',
    'store.password_status_partial': 'Partial',
    'store.password_status_none': 'None',
    'store.no_results': 'No Orbs found.',
    'store.enter_query': 'Enter a search query.',
    'store.download_btn': 'Download',
    'store.downloading': 'Downloading {slug}…',
    'store.downloaded': 'Downloaded and imported {name} {version}',
    'store.results': '{count} result(s)',
    'store.import_btn': 'Import',
    'store.import_btn_downloading': 'Downloading…',
    'store.import_btn_installing': 'Installing…',
    'store.import_btn_done': 'Imported ✓',
    /* running (HTTP gateway) */
    'running.title': 'HTTP',
    'running.refresh': 'Refresh',
    'running.section_desc': 'Gateway HTTP server exposes all installed Orbs as MCP tools through a single endpoint. Copy the config below to connect your MCP client.',
    'running.copy_config_btn': 'Copy Config',
    'running.loading': 'Loading gateway configuration…',
    'running.no_orbs': 'No Orbs installed. The gateway HTTP endpoint is ready — install Orbs from the Library to add tools.',
    'running.gateway_status_running': 'Gateway running · {url}',
    'running.gateway_status_stopped': 'Gateway stopped',
    'running.gateway_start_btn': 'Start Gateway',
    'running.gateway_stop_btn': 'Stop Gateway',
  'running.gateway_starting': 'Starting…',
  'running.gateway_stopping': 'Stopping…',
  'running.gateway_start_failed': 'Gateway failed to start: {error}',
  'running.gateway_stop_failed': 'Failed to stop gateway: {error}',
    'running.gateway_conn_title': 'Connection String (with auth token)',
    'running.gateway_copy_conn': 'Copy Connection String',
    'running.gateway_reset_token': 'Reset Token',
    'running.gateway_resetting': 'Resetting…',
    'running.gateway_token_copied': 'Connection string copied.',
    'running.gateway_reset_confirm': 'Reset the gateway token? Connected MCP clients will lose access until you update their connection string.',
    'running.gateway_reset_failed': 'Failed to reset gateway token: {error}',
    /* settings */
    'settings.title': 'Settings',
    'settings.save_btn': 'Save',
    'settings.http_port_label': 'HTTP MCP Port',
    'settings.network_binding_label': 'Network Binding',
    'settings.localhost_opt': 'Localhost (127.0.0.1) — Recommended',
    'settings.external_opt': 'External (0.0.0.0) — Requires caution',
    'settings.saved': 'Settings saved.',
    'settings.unsaved_hint': 'Unsaved changes — click Save to apply.',
    'settings.orb_library_label': 'Orb Library Folder',
    'settings.orb_library_choose_btn': 'Choose…',
    'settings.orb_library_hint': 'Imported Orb ZIPs are stored in this folder so the files stay accessible to you. For example: ~/Documents/MCPOrb',
    'settings.orb_library_choose_error': 'Could not set the Orb library folder:',
    'settings.orb_library_changed': 'Orb library folder updated.',
    'settings.orb_library_bookmark_stale': 'Orb library folder access has expired (e.g. after an app update). Please click Choose… to re-select the folder.',
    'librarychange.title': 'Change Orb Library Folder?',
    'librarychange.message': '{count} previously imported Orb(s) are stored outside the new library folder. What should happen to them?',
    'librarychange.migrate_btn': 'Migrate Orbs',
    'librarychange.delete_btn': 'Delete Orbs…',
    'librarychange.delete_confirm': 'Delete {count} Orb(s) and their files from the old location? This cannot be undone.',
    'librarychange.cancel_btn': 'Cancel',
    'librarychange.migrated': 'Orb library folder changed. {count} Orb(s) migrated.',
    'librarychange.deleted': 'Orb library folder changed. {count} Orb(s) deleted.',
    'librarychange.error': 'Could not change Orb library folder: {error}',
    /* mcp config */
    'mcp.title': 'MCP Config',
    'mcp.runtime_path_label': 'Runtime CLI path',
    'mcp.runtime_path_placeholder': 'Leave blank to use bundled mcporb-runtime',
    'mcp.generate_btn': 'Generate STDIO Config',
    'mcp.generate_note_windows': 'On Windows, you may need to close the entire AI app from the system tray (notification area) for new MCP settings to take effect.',
    'mcp.platform_config_title': 'Platform MCP Configs',
    'mcp.platform_config_desc': 'Discover and configure MCP servers in LLM platform config files (Claude Desktop, Cursor, VS Code, etc.)',
    'mcp.discover_btn': 'Discover',
    'mcp.discovering': 'Discovering platform configs...',
    'mcp.apply_btn': 'Apply Config',
    'mcp.applying': 'Applying...',
    'mcp.applied': 'Config applied!',
    'mcp.view_raw': 'View Raw',
    'mcp.config_found': 'Found',
    'mcp.config_not_found': 'Not Found',
    'mcp.config_read_error': 'Error',
    'mcp.no_configs': 'No platform configs discovered yet. Click "Discover" to scan for LLM platform config files.',
    'mcp.apply_success': 'Config written to {path}',
    'mcp.apply_success_backup': 'Backup saved to {backup}',
    'mcp.current_label': 'Current',
    'mcp.generated_label': 'MCPOrb Config',
    'mcp.copy_config_btn': 'Copy Config',
    'mcp.restart_hint.claude_desktop': 'Restart Claude Desktop to load the new MCP servers.',
    'mcp.restart_hint.cursor': 'Restart Cursor to load the new MCP servers.',
    'mcp.restart_hint.cline': 'Restart VS Code or reload the Cline extension to load the new MCP servers.',
    'mcp.restart_hint.roo_code': 'Restart VS Code or reload the Roo Code extension to load the new MCP servers.',
    'mcp.restart_hint.windsurf': 'Restart Windsurf to load the new MCP servers.',
    'mcp.restart_hint.zed': 'Restart Zed to load the new MCP servers. Note: Zed uses the `context_servers` format, not `mcpServers`.',
    'mcp.restart_hint.continue_dev': 'Restart Continue to load the new MCP servers. Note: Continue.dev uses array format under `experimental.modelContextProtocolServers`.',
    /* import modal */
    'import.title': 'Import Orb ZIP',
    'import.drop_text': 'Drop Orb ZIP file here',
    'import.browse_btn': 'Browse Files',
    'import.browse_hint': 'or drag & drop a .zip file above',
    'import.cancel_btn': 'Cancel',
    'import.import_btn': 'Import',
    'import.validating': 'Validating and importing Orb ZIP...',
    'import.select_zip': 'Please select a .zip file.',
    'import.desktop_only': 'File selection is only available in the MCPOrb Runner desktop app.',
    'import.success': 'Imported {name} {version}\nStored at {path}',
    'import.password_title': 'Remember Orb Password',
    'import.password_desc': 'Enter the password for "{name}" to remember it on this device.',
    'import.password_submit': 'Save & Remember',
    'import.password_skip': 'Skip',
    'import.password_verifying': 'Verifying password…',
    'import.password_incorrect': 'Incorrect password. Please try again.',
    'import.password_keychain_hint': 'Password is saved to and accessed from your OS credential store (e.g. macOS Keychain, Windows Credential Manager). Stays on this device only.',
    /* status */
    'status.static_preview': 'static preview',
    'status.runtime_unavailable': 'Tauri runtime unavailable',
    'status.store_label': 'Store',
    /* qa */
    'qa.title': 'Service History',
    'qa.close_btn': 'Close',
    'qa.refresh': 'Refresh',
    'qa.no_history': 'No service history yet.',
    'qa.total_requests': 'Total Requests',
    'qa.stdio_requests': 'STDIO',
    'qa.http_requests': 'HTTP',
    'qa.transport': 'Transport',
    'qa.method': 'Method',
    'qa.query': 'Query',
    'qa.response': 'Response',
    'qa.prev': '← Prev',
    'qa.next': 'Next →',
    'qa.page': 'Page {page} of {total}',
    'qa.results': '{count} chunk(s)',
    'qa.loading': 'Loading service history…',
    'qa.error': 'Failed to load service history.',
    'qa.not_running': 'No history data yet.',
    /* orb search modal */
    'orbsearch.search_placeholder': 'Search inside this Orb…',
    'orbsearch.search_btn': 'Search',
    'orbsearch.encrypted_prompt': 'This orb is encrypted. Enter the password above to search.',
    'orbsearch.close_btn': 'Close',
    'orbsearch.no_matches': 'No matches.',
    'orbsearch.enter_query': 'Enter a query to search this Orb.',
    'orbsearch.searching': 'Searching...',
    /* feedback */
    'feedback.refreshed': '✓ Refreshed!',
    'feedback.saved': '✓ Saved!',
    'feedback.generated': '✓ Generated!',
    'feedback.copied': '✓ Copied!',
    'feedback.started': '✓ Started!',
    'feedback.stopped': '✓ Stopped!',
    'feedback.imported': '✓ Imported!',
    'feedback.filtered': '✓ Filtered!',
    'onboarding.title': 'Choose Your Orb Library Location',
    'onboarding.desc': 'To keep your Orb collection visible in Finder and accessible across app updates, MCPOrb Runner stores Orb files in a folder you choose — not inside the hidden app container.',
    'onboarding.recommended': 'Recommended location:',
    'onboarding.hint': 'Click "Open ~/Documents/MCPOrb" — the file picker will open there. Use the New Folder button if the folder doesn\'t exist yet, then click Open.',
    'onboarding.skip_btn': 'Skip for now',
    'onboarding.choose_btn': 'Choose Different Location…',
    'onboarding.default_btn': 'Open ~/Documents/MCPOrb',
    'onboarding.success': 'Orb library folder set to: {path}',
    'onboarding.error': 'Could not set Orb library folder: {error}',
  },
  ja: {
    'app.title': 'MCPOrb Runner',
    'theme.system': 'システム',
    'theme.light': 'ライト',
    'theme.dark': 'ダーク',
    'tab.library': 'ライブラリ',
    'tab.mcp': 'MCP設定',
    'tab.store': 'ストア',
    'tab.http': 'HTTP',
    'tab.settings': '設定',
    'library.title': 'ライブラリ',
    'library.import': 'インポート',
    'library.refresh': '更新',
    'library.filter_placeholder': 'Orbを名前または説明で検索…',
    'library.filter_btn': '絞り込み',
    'library.search_btn': '検索',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': 'インストールされたOrbはありません。「インポート」をクリックしてOrb ZIPを追加してください。',
    'library.no_match': '「{query}」に一致するOrbはありません。',
    'library.qa_btn': 'サービスリクエスト',
    'library.delete_title': 'Orbを削除',
    'library.delete_btn': '削除',
    'library.delete_confirm': '"{name}"を削除してもよろしいですか？',
    'library.delete_success': '{name}を削除しました',
    'library.password_badge': 'パスワード',
    'library.password_every_launch': '起動のたびに',
    'library.password_remembered': '記憶済み',
    'library.restart_hint': 'MCPクライアント（Claude、Cursorなど）が起動中の場合は、再起動してOrbの変更を反映してください。',
    'library.bookmark_stale_banner': 'Orbライブラリフォルダへのアクセスが無効になりました（例：アプリ更新後）。フォルダを再選択するまでOrbは利用できません。',
    'library.bookmark_stale_fix_btn': '再選択…',
    'library.stats_requests': 'リクエスト: {total}',
    'library.page_info': '{page}/{total} ページ',
    'store.title': 'ストア',
    'store.search_placeholder': 'MCP StoreでOrbを検索',
    'store.search_btn': '検索',
    'store.detail_title': 'Orbの詳細',
    'store.detail_back_btn': '戻る',
    'store.version_label': 'バージョン',
    'store.versions_title': 'バージョン',
    'store.artifacts_title': 'アーティファクト',
    'store.download_zip_btn': 'ZIPをダウンロード',
    'store.password_label': 'パスワード',
    'store.password_placeholder': 'パスワードを入力',
    'store.password_submit': '送信',
    'store.password_required_title': 'パスワードが必要です',
    'store.password_required_label': 'このアーティファクトのパスワードを入力してください:',
    'store.password_verifying': '確認中...',
    'store.password_incorrect': 'ダウンロードパスワードが間違っています。もう一度お試しください。',
    'store.password_network_error': 'ストアサーバーに接続できませんでした。ネットワークを確認してもう一度お試しください。',
    'store.download_error': 'ダウンロードエラー: {error}',
    'store.artifact_downloaded': 'ダウンロードしました: {result}',
    'store.tags_filter': 'タグ',
    'store.tag_all': 'すべてのタグ',
    'store.method_filter': '方法',
    'store.method_all': 'すべての方法',
    'store.no_detail': '詳細はありません。',
    'store.artifact_kind_canonical': '正規',
    'store.artifact_kind_derived': '派生',
    'store.password_status_all': 'すべて',
    'store.password_status_partial': '一部',
    'store.password_status_none': 'なし',
    'store.no_results': 'Orbが見つかりません。',
    'store.enter_query': '検索クエリを入力してください。',
    'store.download_btn': 'ダウンロード',
    'store.downloading': '{slug}をダウンロード中…',
    'store.import_btn': 'Import',
    'store.import_btn_downloading': 'ダウンロード中…',
    'store.import_btn_installing': 'インストール中…',
    'store.import_btn_done': '✓ 完了',
    'store.downloaded': '{name} {version}をダウンロードしてインポートしました',
    'store.results': '{count}件の結果',
    'running.title': 'HTTP',
    'running.refresh': '更新',
    'running.section_desc': 'ゲートウェイHTTPサーバーは、インストール済みの全Orbを単一のエンドポイントでMCPツールとして公開します。以下の設定をコピーしてMCPクライアントに接続してください。',
    'running.copy_config_btn': '設定をコピー',
    'running.loading': 'ゲートウェイ設定を読み込み中…',
    'running.no_orbs': 'Orbがインストールされていません。ライブラリからOrbをインストールすると、ゲートウェイHTTPエンドポイントが利用可能になります。',
    'running.gateway_status_running': 'ゲートウェイ実行中 · {url}',
    'running.gateway_status_stopped': 'ゲートウェイ停止中',
    'running.gateway_start_btn': 'ゲートウェイを起動',
    'running.gateway_stop_btn': 'ゲートウェイを停止',
  'running.gateway_starting': '起動中…',
  'running.gateway_stopping': '停止中…',
  'running.gateway_start_failed': 'ゲートウェイの起動に失敗しました: {error}',
  'running.gateway_stop_failed': 'ゲートウェイの停止に失敗しました: {error}',
    'running.gateway_conn_title': '接続文字列（認証トークン付き）',
    'running.gateway_copy_conn': '接続文字列をコピー',
    'running.gateway_reset_token': 'トークンを再発行',
    'running.gateway_resetting': '再発行中…',
    'running.gateway_token_copied': '接続文字列をコピーしました。',
    'running.gateway_reset_confirm': 'ゲートウェイトークンを再発行しますか？ 接続中のMCPクライアントは、接続文字列を更新するまでアクセスできなくなります。',
    'running.gateway_reset_failed': 'ゲートウェイトークンの再発行に失敗しました: {error}',
    /* settings */
    'settings.save_btn': '保存',
    'settings.http_port_label': 'HTTP MCPポート',
    'settings.network_binding_label': 'ネットワークバインディング',
    'settings.localhost_opt': 'ローカルホスト (127.0.0.1) — 推奨',
    'settings.external_opt': '外部 (0.0.0.0) — 注意が必要',
    'settings.yes_opt': 'はい',
    'settings.no_opt': 'いいえ',
    'settings.saved': '設定を保存しました。',
    'settings.unsaved_hint': '未保存の変更があります — 保存をクリックして適用してください。',
    'settings.orb_library_label': 'Orbライブラリフォルダ',
    'settings.orb_library_choose_btn': '選択…',
    'settings.orb_library_hint': 'インポートしたOrb ZIPはこのフォルダに保存され、ファイルにアクセスできる状態が維持されます。例: ~/Documents/MCPOrb',
    'settings.orb_library_choose_error': 'Orbライブラリフォルダを設定できませんでした:',
    'settings.orb_library_changed': 'Orbライブラリフォルダを更新しました。',
    'settings.orb_library_bookmark_stale': 'Orbライブラリフォルダへのアクセスが無効になりました（例：アプリ更新後）。「選択…」をクリックしてフォルダを再選択してください。',
    'librarychange.title': 'Orbライブラリフォルダを変更しますか？',
    'librarychange.message': '新しいライブラリフォルダの外に、以前インポートした {count} 個のOrbがあります。どうしますか？',
    'librarychange.migrate_btn': 'Orbを移行',
    'librarychange.delete_btn': 'Orbを削除…',
    'librarychange.delete_confirm': '以前の場所から {count} 個のOrbとそのファイルを削除しますか？この操作は元に戻せません。',
    'librarychange.cancel_btn': 'キャンセル',
    'librarychange.migrated': 'Orbライブラリフォルダを変更しました。{count} 個のOrbを移行しました。',
    'librarychange.deleted': 'Orbライブラリフォルダを変更しました。{count} 個のOrbを削除しました。',
    'librarychange.error': 'Orbライブラリフォルダを変更できませんでした: {error}',
    'mcp.title': 'MCP設定',
    'mcp.runtime_path_label': 'ランタイムCLIパス',
    'mcp.runtime_path_placeholder': '空白の場合はバンドルされたmcporb-runtimeを使用',
    'mcp.generate_btn': 'STDIO設定を生成',
    'mcp.generate_note_windows': 'Windowsでは、新しいMCP設定を有効にするために、タスクトレイからAIアプリケーション全体を終了する必要がある場合があります。',
    'mcp.platform_config_title': 'プラットフォームMCP設定',
    'mcp.platform_config_desc': 'LLMプラットフォーム（Claude Desktop、Cursor、VS Codeなど）の設定ファイルを検出し、MCPサーバーを構成します。',
    'mcp.discover_btn': '検出',
    'mcp.discovering': 'プラットフォーム設定を検出中...',
    'mcp.apply_btn': '設定を適用',
    'mcp.applying': '適用中...',
    'mcp.applied': '設定を適用しました！',
    'mcp.view_raw': '生データ表示',
    'mcp.config_found': '見つかりました',
    'mcp.config_not_found': '見つかりません',
    'mcp.config_read_error': 'エラー',
    'mcp.no_configs': 'プラットフォーム設定がまだ検出されていません。「検出」をクリックしてLLMプラットフォームの設定ファイルをスキャンしてください。',
    'mcp.apply_success': '設定を {path} に書き込みました',
    'mcp.apply_success_backup': 'バックアップを {backup} に保存しました',
    'mcp.current_label': '現在の設定',
    'mcp.generated_label': 'MCPOrb設定',
    'mcp.copy_config_btn': '設定をコピー',
    'mcp.restart_hint.claude_desktop': 'Claude Desktopを再起動して新しいMCPサーバーを読み込んでください。',
    'mcp.restart_hint.cursor': 'Cursorを再起動して新しいMCPサーバーを読み込んでください。',
    'mcp.restart_hint.cline': 'VS Codeを再起動するか、Cline拡張機能をリロードして新しいMCPサーバーを読み込んでください。',
    'mcp.restart_hint.roo_code': 'VS Codeを再起動するか、Roo Code拡張機能をリロードして新しいMCPサーバーを読み込んでください。',
    'mcp.restart_hint.windsurf': 'Windsurfを再起動して新しいMCPサーバーを読み込んでください。',
    'mcp.restart_hint.zed': 'Zedを再起動して新しいMCPサーバーを読み込んでください。注：Zedは`context_servers`形式を使用し、`mcpServers`形式ではありません。',
    'mcp.restart_hint.continue_dev': 'Continueを再起動して新しいMCPサーバーを読み込んでください。注：Continue.devは`experimental.modelContextProtocolServers`配列形式を使用します。',
    'import.title': 'Orb ZIPをインポート',
    'import.drop_text': 'Orb ZIPファイルをここにドロップ',
    'import.browse_btn': 'ファイルを選択',
    'import.browse_hint': 'または上に.zipファイルをドラッグ＆ドロップ',
    'import.cancel_btn': 'キャンセル',
    'import.import_btn': 'インポート',
    'import.validating': 'Orb ZIPを検証・インポート中...',
    'import.select_zip': '.zipファイルを選択してください。',
    'import.desktop_only': 'ファイル選択はMCPOrb Runnerデスクトップアプリでのみ利用可能です。',
    'import.success': '{name} {version}をインポートしました\n保存先: {path}',
    'import.password_title': 'Orbパスワードを保存',
    'import.password_desc': '「{name}」のパスワードをこのデバイスに保存します。',
    'import.password_submit': '保存する',
    'import.password_skip': 'スキップ',
    'import.password_verifying': 'パスワードを確認中…',
    'import.password_incorrect': 'パスワードが間違っています。もう一度お試しください。',
    'import.password_keychain_hint': 'パスワードはOSの認証情報ストア（macOSキーチェーン、Windows資格情報マネージャーなど）に保存・アクセスされます。このデバイスでのみ保持されます。',
    /* status */
    'status.static_preview': 'static preview',
    'status.runtime_unavailable': 'Tauri runtime unavailable',
    'status.store_label': 'Store',
    /* qa */
    'qa.title': 'サービス履歴',
    'qa.close_btn': '閉じる',
    'qa.refresh': '更新',
    'qa.no_history': 'サービス履歴はまだありません。',
    'qa.total_requests': '総リクエスト数',
    'qa.stdio_requests': 'STDIO',
    'qa.http_requests': 'HTTP',
    'qa.transport': 'トランスポート',
    'qa.method': 'メソッド',
    'qa.query': 'クエリ',
    'qa.response': 'レスポンス',
    'qa.prev': '← 前へ',
    'qa.next': '次へ →',
    'qa.page': '{page}/{total}ページ',
    'qa.results': '{count} chunk',
    'qa.loading': 'サービス履歴を読み込み中…',
    'qa.error': 'サービス履歴の読み込みに失敗しました。',
    'qa.not_running': '履歴データがまだありません。',
    'orbsearch.title': 'Orbを検索',
    'orbsearch.search_placeholder': 'このOrb内を検索…',
    'orbsearch.search_btn': '検索',
    'orbsearch.close_btn': '閉じる',
    'orbsearch.no_matches': '一致しませんでした。',
    'orbsearch.enter_query': 'このOrbを検索するクエリを入力してください。',
    'orbsearch.searching': '検索中...',
    'feedback.refreshed': '✓ 更新しました！',
    'feedback.saved': '✓ 保存しました！',
    'feedback.generated': '✓ 生成しました！',
    'feedback.copied': '✓ コピーしました！',
    'feedback.started': '✓ 起動しました！',
    'feedback.stopped': '✓ 停止しました！',
    'feedback.imported': '✓ インポートしました！',
    'feedback.filtered': '✓ 絞り込みました！',
    'onboarding.title': 'Orbライブラリの保存場所を選択',
    'onboarding.desc': 'FinderからOrbファイルにアクセスできるよう、MCPOrb Runnerはアプリのコンテナではなく、指定したフォルダにOrbを保存します。',
    'onboarding.recommended': '推奨保存場所:',
    'onboarding.hint': '「~/Documents/MCPOrb を開く」をクリックするとファイルピッカーがそのフォルダで開きます。フォルダがまだ存在しない場合は「新規フォルダ」で作成して選択してください。',
    'onboarding.skip_btn': '後で設定する',
    'onboarding.choose_btn': '別の場所を選択…',
    'onboarding.default_btn': '~/Documents/MCPOrb を開く',
    'onboarding.success': 'Orbライブラリフォルダを設定しました: {path}',
    'onboarding.error': 'Orbライブラリフォルダを設定できませんでした: {error}',
  },
  zh: {
    'app.title': 'MCPOrb Runner',
    'theme.system': '跟随系统',
    'theme.light': '浅色',
    'theme.dark': '深色',
    'tab.library': '库',
    'tab.mcp': 'MCP配置',
    'tab.store': '商店',
    'tab.http': 'HTTP',
    'tab.settings': '设置',
    'library.title': '库',
    'library.import': '导入',
    'library.refresh': '刷新',
    'library.filter_placeholder': '按名称或描述搜索Orb…',
    'library.filter_btn': '筛选',
    'library.search_btn': '搜索',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': '尚未安装Orb。点击"导入"添加Orb ZIP。',
    'library.no_match': '没有匹配"{query}"的Orb。',
    'library.qa_btn': '服务请求',
    'library.delete_title': '删除Orb',
    'library.delete_btn': '删除',
    'library.delete_confirm': '确定要删除"{name}"吗？',
    'library.delete_success': '已删除{name}',
    'library.password_badge': '密码',
    'library.password_every_launch': '每次启动',
    'library.password_remembered': '已记住',
    'library.restart_hint': '如有正在运行中的MCP客户端（Claude、Cursor等），请重启该客户端以使用更新后的Orb。',
    'library.bookmark_stale_banner': 'Orb 库文件夹访问已失效（例如应用更新后）。重新选择文件夹前，Orb 不可用。',
    'library.bookmark_stale_fix_btn': '重新选择…',
    'library.stats_requests': '请求: {total}',
    'library.stats_searches': '搜索: {n}',
    'library.stats_stdio': 'STDIO: {n}',
    'library.stats_http': 'HTTP: {n}',
    'library.page_info': '第{page}/{total}页',
    'store.title': '商店',
    'store.search_placeholder': '在MCP商店中搜索Orb',
    'store.search_btn': '搜索',
    'store.detail_title': 'Orb 详情',
    'store.detail_back_btn': '返回',
    'store.version_label': '版本',
    'store.versions_title': '版本',
    'store.artifacts_title': '构件',
    'store.download_zip_btn': '下载 ZIP',
    'store.password_label': '密码',
    'store.password_placeholder': '请输入密码',
    'store.password_submit': '提交',
    'store.password_required_title': '需要密码',
    'store.password_required_label': '请输入此构件的密码:',
    'store.password_verifying': '正在验证...',
    'store.password_incorrect': '下载密码错误，请重试。',
    'store.password_network_error': '无法连接到商店服务器，请检查网络后重试。',
    'store.download_error': '下载错误: {error}',
    'store.artifact_downloaded': '已下载: {result}',
    'store.tags_filter': '标签',
    'store.tag_all': '全部标签',
    'store.method_filter': '方法',
    'store.method_all': '全部方法',
    'store.no_detail': '暂无详情。',
    'store.artifact_kind_canonical': '标准',
    'store.artifact_kind_derived': '派生',
    'store.password_status_all': '全部',
    'store.password_status_partial': '部分',
    'store.password_status_none': '无',
    'store.no_results': '未找到Orb。',
    'store.enter_query': '请输入搜索查询。',
    'store.download_btn': '下载',
    'store.downloading': '正在下载{slug}…',
    'store.import_btn': '导入',
    'store.import_btn_downloading': '正在下载…',
    'store.import_btn_installing': '正在安装…',
    'store.import_btn_done': '已导入 ✓',
    'store.downloaded': '已下载并导入{name} {version}',
    'store.results': '{count}个结果',
    'running.title': 'HTTP',
    'running.refresh': '刷新',
    'running.section_desc': '网关HTTP服务器将所有已安装的Orb通过单一端点暴露为MCP工具。复制以下配置连接到您的MCP客户端。',
    'running.copy_config_btn': '复制配置',
    'running.loading': '正在加载网关配置…',
    'running.no_orbs': '尚未安装Orb。从库中安装Orb后，网关HTTP端点即可使用。',
    'running.gateway_status_running': '网关运行中 · {url}',
    'running.gateway_status_stopped': '网关已停止',
    'running.gateway_start_btn': '启动网关',
    'running.gateway_stop_btn': '停止网关',
  'running.gateway_starting': '启动中…',
  'running.gateway_stopping': '停止中…',
  'running.gateway_start_failed': '网关启动失败: {error}',
  'running.gateway_stop_failed': '停止网关失败: {error}',
    'running.gateway_conn_title': '连接字符串（含认证令牌）',
    'running.gateway_copy_conn': '复制连接字符串',
    'running.gateway_reset_token': '重置令牌',
    'running.gateway_resetting': '重置中…',
    'running.gateway_token_copied': '连接字符串已复制。',
    'running.gateway_reset_confirm': '确定重置网关令牌吗？已连接的 MCP 客户端在更新连接字符串之前将无法访问。',
    'running.gateway_reset_failed': '重置网关令牌失败: {error}',
    'settings.title': '设置',
    'settings.save_btn': '保存',
    'settings.http_port_label': 'HTTP MCP端口',
    'settings.network_binding_label': '网络绑定',
    'settings.localhost_opt': '本地主机 (127.0.0.1) — 推荐',
    'settings.external_opt': '外部 (0.0.0.0) — 需谨慎',
    'settings.yes_opt': '是',
    'settings.no_opt': '否',
    'settings.saved': '设置已保存。',
    'settings.unsaved_hint': '有未保存的更改 — 点击保存以生效。',
    'settings.orb_library_label': 'Orb 库文件夹',
    'settings.orb_library_choose_btn': '选择…',
    'settings.orb_library_hint': '导入的 Orb ZIP 将存储在此文件夹中,文件保持对你可见。例如:~/Documents/MCPOrb',
    'settings.orb_library_choose_error': '无法设置 Orb 库文件夹:',
    'settings.orb_library_changed': 'Orb 库文件夹已更新。',
    'settings.orb_library_bookmark_stale': 'Orb 库文件夹访问已失效（例如应用更新后）。请点击"选择…"重新选择文件夹。',
    'librarychange.title': '更改 Orb 库文件夹？',
    'librarychange.message': '新库文件夹之外有 {count} 个之前导入的 Orb。如何处理它们？',
    'librarychange.migrate_btn': '迁移 Orb',
    'librarychange.delete_btn': '删除 Orb…',
    'librarychange.delete_confirm': '从旧位置删除 {count} 个 Orb 及其文件？此操作无法撤销。',
    'librarychange.cancel_btn': '取消',
    'librarychange.migrated': 'Orb 库文件夹已更改。已迁移 {count} 个 Orb。',
    'librarychange.deleted': 'Orb 库文件夹已更改。已删除 {count} 个 Orb。',
    'librarychange.error': '无法更改 Orb 库文件夹: {error}',
    'mcp.title': 'MCP配置',
    'mcp.runtime_path_label': '运行时CLI路径',
    'mcp.runtime_path_placeholder': '留空使用内置mcporb-runtime',
    'mcp.generate_btn': '生成STDIO配置',
    'mcp.generate_note_windows': 'Windows中可能需要从任务栏（Tray）关闭整个AI应用，以使新的MCP设定生效。',
    'mcp.platform_config_title': '平台MCP配置',
    'mcp.platform_config_desc': '发现并配置LLM平台(Claude Desktop、Cursor、VS Code等)的MCP服务器配置文件。',
    'mcp.discover_btn': '发现',
    'mcp.discovering': '正在发现平台配置文件...',
    'mcp.apply_btn': '应用配置',
    'mcp.applying': '正在应用...',
    'mcp.applied': '配置已应用！',
    'mcp.view_raw': '查看原始内容',
    'mcp.config_found': '已找到',
    'mcp.config_not_found': '未找到',
    'mcp.config_read_error': '错误',
    'mcp.no_configs': '尚未发现平台配置。点击"发现"扫描LLM平台的配置文件。',
    'mcp.apply_success': '配置已写入 {path}',
    'mcp.apply_success_backup': '备份已保存到 {backup}',
    'mcp.current_label': '当前配置',
    'mcp.generated_label': 'MCPOrb配置',
    'mcp.copy_config_btn': '复制配置',
    'mcp.restart_hint.claude_desktop': '重启 Claude Desktop 以加载新的 MCP 服务器。',
    'mcp.restart_hint.cursor': '重启 Cursor 以加载新的 MCP 服务器。',
    'mcp.restart_hint.cline': '重启 VS Code 或重新加载 Cline 扩展以加载新的 MCP 服务器。',
    'mcp.restart_hint.roo_code': '重启 VS Code 或重新加载 Roo Code 扩展以加载新的 MCP 服务器。',
    'mcp.restart_hint.windsurf': '重启 Windsurf 以加载新的 MCP 服务器。',
    'mcp.restart_hint.zed': '重启 Zed 以加载新的 MCP 服务器。注意：Zed 使用 `context_servers` 格式，而非 `mcpServers`。',
    'mcp.restart_hint.continue_dev': '重启 Continue 以加载新的 MCP 服务器。注意：Continue.dev 使用 `experimental.modelContextProtocolServers` 数组格式。',
    'import.title': '导入Orb ZIP',
    'import.drop_text': '将Orb ZIP文件拖放到此处',
    'import.browse_btn': '浏览文件',
    'import.browse_hint': '或将.zip文件拖放到上方',
    'import.cancel_btn': '取消',
    'import.import_btn': '导入',
    'import.validating': '正在验证并导入Orb ZIP...',
    'import.select_zip': '请选择.zip文件。',
    'import.desktop_only': '文件选择仅在MCPOrb Runner桌面应用中可用。',
    'import.success': '已导入{name} {version}\n存储位置: {path}',
    'import.password_title': '记住 Orb 密码',
    'import.password_desc': '输入「{name}」的密码以保存到本设备。',
    'import.password_submit': '保存并记住',
    'import.password_skip': '跳过',
    'import.password_verifying': '验证密码中…',
    'import.password_incorrect': '密码错误，请重试。',
    'import.password_keychain_hint': '密码将保存到操作系统凭据存储（如 macOS 钥匙串、Windows 凭据管理器）并从同一存储访问。仅保留在此设备上。',
    /* status */
    'status.static_preview': 'static preview',
    'status.runtime_unavailable': 'Tauri runtime unavailable',
    'status.store_label': 'Store',
    /* qa */
    'qa.title': '服务记录',
    'qa.close_btn': '关闭',
    'qa.refresh': '刷新',
    'qa.no_history': '暂无服务记录。',
    'qa.total_requests': '总请求数',
    'qa.stdio_requests': 'STDIO',
    'qa.http_requests': 'HTTP',
    'qa.transport': '传输方式',
    'qa.method': '方法',
    'qa.query': '问题',
    'qa.response': '回答',
    'qa.prev': '← 上一页',
    'qa.next': '下一页 →',
    'qa.page': '第{page}/{total}页',
    'qa.results': '{count} 个 chunk',
    'qa.loading': '加载服务记录中…',
    'qa.error': '加载服务记录失败。',
    'qa.not_running': '暂无历史数据。',
    'orbsearch.title': '搜索Orb',
    'orbsearch.search_placeholder': '在此Orb内搜索…',
    'orbsearch.search_btn': '搜索',
    'orbsearch.close_btn': '关闭',
    'orbsearch.no_matches': '无匹配结果。',
    'orbsearch.enter_query': '请输入搜索此Orb的查询。',
    'orbsearch.searching': '搜索中...',
    'feedback.refreshed': '✓ 已刷新！',
    'feedback.saved': '✓ 已保存！',
    'feedback.generated': '✓ 已生成！',
    'feedback.copied': '✓ 已复制！',
    'feedback.started': '✓ 已启动！',
    'feedback.stopped': '✓ 已停止！',
    'feedback.imported': '✓ 已导入！',
    'feedback.filtered': '✓ 已筛选！',
    'onboarding.title': '选择 Orb 库文件夹',
    'onboarding.desc': '为了让你的 Orb 文件在 Finder 中可见并在应用更新后保持可访问，MCPOrb Runner 会将 Orb 文件保存到你指定的文件夹，而不是隐藏的应用容器中。',
    'onboarding.recommended': '推荐位置：',
    'onboarding.hint': '点击"打开 ~/Documents/MCPOrb"，文件选择器将在该位置打开。如果文件夹尚不存在，请使用"新建文件夹"创建后再选择。',
    'onboarding.skip_btn': '稍后设置',
    'onboarding.choose_btn': '选择其他位置…',
    'onboarding.default_btn': '打开 ~/Documents/MCPOrb',
    'onboarding.success': '已设置 Orb 库文件夹：{path}',
    'onboarding.error': '无法设置 Orb 库文件夹：{error}',
  },
};

let locale = 'en';

function t(key, params) {
  const msg = (locales[locale] && locales[locale][key]) || locales.en[key] || key;
  if (!params) return msg;
  return msg.replace(/\{(\w+)\}/g, (_, k) => (params[k] != null ? params[k] : `{${k}}`));
}

function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

function applyLocale() {
  // Static elements with data-i18n
  document.querySelectorAll('[data-i18n]').forEach((el) => {
    const key = el.getAttribute('data-i18n');
    el.textContent = t(key);
  });
  document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
    const key = el.getAttribute('data-i18n-placeholder');
    el.placeholder = t(key);
  });
  document.querySelectorAll('[data-i18n-label]').forEach((el) => {
    const key = el.getAttribute('data-i18n-label');
    const label = el.querySelector('label');
    if (label) label.textContent = t(key);
  });
  document.title = t('app.title');
  // Re-render dynamic lists
  if (state.orbs.length) renderLibrary(state.orbs);
  if ($('running-list').children.length) refreshRunning();
  if (state.platformConfigs.length) renderPlatformConfigs(state.platformConfigs);
  // Update restart hint text if visible
  const hint = $('library-restart-hint');
  if (hint && hint.style.display !== 'none') {
    $('library-restart-hint-text').textContent = t('library.restart_hint');
  }
  updateUnsavedHint();
}

function initLocale() {
  const saved = localStorage.getItem(LOCALE_KEY);
  if (saved && locales[saved]) locale = saved;
  const sel = $('lang-select');
  if (sel) sel.value = locale;
  applyLocale();
}

function setLocale(code) {
  if (!locales[code]) return;
  locale = code;
  localStorage.setItem(LOCALE_KEY, code);
  applyLocale();
}

// ── Button feedback helper ────────────────────────────────────────────────

function feedbackBtn(el, feedbackKey, duration) {
  if (!el) return;
  const orig = el.textContent;
  el.textContent = t(feedbackKey);
  el.disabled = true;
  setTimeout(() => {
    el.textContent = orig;
    el.disabled = false;
  }, duration || 1200);
}

// ── Auto-refresh library (30s interval, only when tab is active) ──────────

const LIBRARY_AUTO_REFRESH_MS = 30000;

async function refreshLibrarySilent() {
  if (!invoke) return;
  try {
    state.orbs = await invoke('list_orbs');
    renderLibrary(state.orbs);
    syncOrbSelects();
  } catch (error) {
    // silent — don't spam the UI on transient errors
  }
}

window.addEventListener('DOMContentLoaded', async () => {
  initTheme();
  initLocale();
  $('lang-select').addEventListener('change', (e) => setLocale(e.target.value));
  const themeSel = $('theme-select');
  if (themeSel) themeSel.addEventListener('change', (e) => setTheme(e.target.value));
  bindTabs();
  bindActions();
  setupDragDrop();
  await loadStatus();
  await refreshLibrary();
  await loadSettings();
  await checkLibraryHealth();
  await refreshRunning();
  await discoverPlatformConfigs();

  // Start auto-refresh timer — checks active tab each cycle
  setInterval(() => {
    if (state.activeTab === 'library') {
      refreshLibrarySilent();
    }
  }, LIBRARY_AUTO_REFRESH_MS);
});

function bindTabs() {
  document.querySelectorAll('.tab-item').forEach((button) => {
    button.addEventListener('click', () => showTab(button.dataset.tab));
  });
}

function bindActions() {
  $('btn-refresh-library').addEventListener('click', refreshLibrary);
  $('btn-show-import').addEventListener('click', showImportModal);
  $('btn-filter-library').addEventListener('click', filterLibrary);
  $('filter-query').addEventListener('input', debounce(filterLibrary, 250));
  $('filter-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') filterLibrary();
  });
  $('btn-generate-config').addEventListener('click', generateMcpConfig);
  $('btn-save-settings').addEventListener('click', saveSettings);
  $('btn-choose-orb-library').addEventListener('click', chooseOrbLibraryDir);
  const fixBtn = $('btn-fix-library-bookmark');
  if (fixBtn) fixBtn.addEventListener('click', fixLibraryBookmark);
  ['settings-http-port', 'settings-network-binding'].forEach((id) => {
    const el = $(id);
    if (el) el.addEventListener('input', markSettingsEdited);
  });
  $('btn-onboarding-default').addEventListener('click', onboardingUseDefault);
  $('btn-onboarding-choose').addEventListener('click', onboardingChooseDifferent);
  $('btn-onboarding-skip').addEventListener('click', onboardingSkip);
  $('btn-refresh-running').addEventListener('click', refreshRunning);
  $('btn-discover-configs').addEventListener('click', discoverPlatformConfigs);
  $('store-search-query').addEventListener('input', debounce(() => {
    state.storeSearchState.query = $('store-search-query').value.trim();
    state.storeSearchState.page = 1;
    storeSearch();
  }, 300));
  $('store-search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') storeSearch();
  });
  // Tag filter dropdown: trigger toggle
  $('store-tag-filter-trigger').addEventListener('click', (e) => {
    e.stopPropagation();
    toggleTagDropdown();
  });
  // Tag filter dropdown: item selection via delegation
  $('store-tag-filter-list').addEventListener('click', (e) => {
    const item = e.target.closest('.tag-filter-item');
    if (item) onTagFilterItemClick(item.dataset.tagSlug);
  });
  // Tag filter dropdown: filter input
  $('store-tag-filter-input').addEventListener('input', () => {
    renderTagFilterList(storeTags, $('store-tag-filter-input').value);
  });
  // Tag filter dropdown: clear filter button
  $('store-tag-filter-clear').addEventListener('click', () => {
    const input = $('store-tag-filter-input');
    input.value = '';
    setStoreTag(null);
    renderTagFilterList(storeTags, '');
    input.focus();
    storeSearch();
  });
  // Close tag dropdown on outside click
  document.addEventListener('click', (e) => {
    const wrapper = $('store-tag-filter-trigger')?.closest('.tag-filter-wrapper');
    if (wrapper && !wrapper.contains(e.target)) {
      toggleTagDropdown(false);
    }
  });
  $('store-method-filter').addEventListener('change', storeSearch);
  $('btn-store-password-submit').addEventListener('click', storeSubmitPassword);
  $('btn-store-password-cancel').addEventListener('click', storeCancelPassword);
  $('store-password-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') storeSubmitPassword();
  });
  $('store-password-dialog').addEventListener('click', (event) => {
    if (event.target === $('store-password-dialog')) storeCancelPassword();
  });
  $('btn-import-password-submit').addEventListener('click', submitImportPassword);
  $('btn-import-password-cancel').addEventListener('click', cancelImportPassword);
  $('btn-import-password-close').addEventListener('click', cancelImportPassword);
  $('import-password-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') submitImportPassword();
  });
  $('import-password-dialog').addEventListener('click', (event) => {
    if (event.target === $('import-password-dialog')) cancelImportPassword();
  });
  // Modal actions
  $('btn-modal-close').addEventListener('click', hideImportModal);
  $('btn-modal-cancel').addEventListener('click', hideImportModal);
  $('btn-modal-import').addEventListener('click', confirmImport);
  $('btn-clear-file').addEventListener('click', clearSelectedFile);
  $('btn-browse').addEventListener('click', browseFile);
  $('drop-zone').addEventListener('click', browseFile);
  // Close modal on overlay click
  $('import-modal').addEventListener('click', (e) => {
    if (e.target === $('import-modal')) hideImportModal();
  });
  // Esc key to close modals
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && importModalVisible()) hideImportModal();
    if (e.key === 'Escape' && orbSearchModalVisible()) hideOrbSearchModal();
    if (e.key === 'Escape' && qaModalVisible()) hideQaModal();
    if (e.key === 'Escape' && confirmModalVisible()) hideConfirmDeleteModal();
    if (e.key === 'Escape' && storePasswordDialogVisible()) storeCancelPassword();
    if (e.key === 'Escape' && state.activeTab === 'store' && state.storeView === 'detail') showStoreBrowse();
    if (e.key === 'Escape') toggleTagDropdown(false);
    // Cmd/Ctrl+K to focus store search
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      if (state.activeTab === 'store') {
        $('store-search-query').focus();
      }
    }
  });
  // Orb search modal
  $('btn-orb-search-go').addEventListener('click', runOrbSearch);
  $('btn-orb-search-pw-submit').addEventListener('click', submitOrbSearchPassword);
  $('orb-search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') runOrbSearch();
  });
  $('orb-search-pw-input').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') submitOrbSearchPassword();
  });
  $('btn-orb-search-close').addEventListener('click', hideOrbSearchModal);
  $('btn-orb-search-cancel').addEventListener('click', hideOrbSearchModal);
  $('orb-search-modal').addEventListener('click', (e) => {
    if (e.target === $('orb-search-modal')) hideOrbSearchModal();
  });
  // Q&A modal
  $('btn-qa-refresh').addEventListener('click', () => {
    if (state.qaOrbId) loadQaModalData(state.qaOrbId);
  });
  $('btn-qa-close').addEventListener('click', hideQaModal);
  $('btn-qa-cancel').addEventListener('click', hideQaModal);
  $('qa-modal').addEventListener('click', (e) => {
    if (e.target === $('qa-modal')) hideQaModal();
  });
  $('btn-confirm-delete').addEventListener('click', confirmDeleteOrb);
  $('btn-confirm-cancel').addEventListener('click', hideConfirmDeleteModal);
  $('btn-confirm-close').addEventListener('click', hideConfirmDeleteModal);
  $('confirm-modal').addEventListener('click', (e) => {
    if (e.target === $('confirm-modal')) hideConfirmDeleteModal();
  });
  // Orb library folder change modal
  $('btn-library-change-migrate').addEventListener('click', migrateLibraryChange);
  $('btn-library-change-delete').addEventListener('click', deleteLibraryChange);
  $('btn-library-change-cancel').addEventListener('click', cancelLibraryChange);
  $('btn-library-change-close').addEventListener('click', cancelLibraryChange);
  $('library-change-modal').addEventListener('click', (e) => {
    if (e.target === $('library-change-modal')) cancelLibraryChange();
  });
  // Restart hint dismiss
  $('btn-dismiss-restart-hint').addEventListener('click', hideRestartHint);
}

function setupDragDrop() {
  const dropZone = $('drop-zone');
  if (!dropZone) return;

  dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropZone.classList.add('drag-over');
  });

  dropZone.addEventListener('dragleave', () => {
    dropZone.classList.remove('drag-over');
  });

  dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropZone.classList.remove('drag-over');
    if (e.dataTransfer?.files?.length > 0) {
      const file = e.dataTransfer.files[0];
      if (!file.name.toLowerCase().endsWith('.zip')) {
        setModalStatus(t('import.select_zip'), true);
        return;
      }
    }
  });

  // Listen for Tauri drag-drop which provides the real filesystem path
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('tauri://drag-drop', (event) => {
      if (!importModalVisible()) return;
      const paths = event.payload?.paths || [];
      if (paths.length > 0) {
        handleFileSelected(paths[0]);
      }
    });
  }
}

function importModalVisible() {
  return $('import-modal').style.display !== 'none';
}

function showImportModal() {
  importState.selectedPath = null;
  $('selected-file').style.display = 'none';
  setModalStatus('', false);
  $('btn-modal-import').disabled = true;
  $('drop-zone').style.display = '';
  $('import-modal').style.display = '';
}

function hideImportModal() {
  $('import-modal').style.display = 'none';
}

async function browseFile() {
  try {
    let path = null;
    // Direct IPC: window.__TAURI__.dialog is unavailable in this plain-JS frontend.
    if (invoke) {
      path = await invoke('plugin:dialog|open', {
        options: {
          multiple: false,
          filters: [{ name: 'Orb ZIP', extensions: ['zip'] }],
        },
      });
    } else {
      setModalStatus(t('import.desktop_only'), true);
      return;
    }
    if (path) handleFileSelected(path);
  } catch (error) {
    setModalStatus(String(error), true);
  }
}

function handleFileSelected(path) {
  if (typeof path !== 'string') return;
  if (!path.toLowerCase().endsWith('.zip')) {
    setModalStatus(t('import.select_zip'), true);
    return;
  }
  importState.selectedPath = path;
  const name = path.split(/[\\/]/).pop() || path;
  $('drop-zone').style.display = 'none';
  $('selected-file-name').textContent = name;
  $('selected-file').style.display = 'flex';
  $('btn-modal-import').disabled = false;
  setModalStatus('', false);
}

function clearSelectedFile() {
  importState.selectedPath = null;
  $('selected-file').style.display = 'none';
  $('drop-zone').style.display = '';
  $('btn-modal-import').disabled = true;
}

async function doImport(path, password) {
  feedbackBtn($('btn-modal-import'), 'feedback.imported');
  setModalStatus(t('import.validating'), false);
  try {
    const result = await invoke('import_orb_zip', { path, password });
    const orbName = result.report.manifest.display_name || result.report.manifest.name;
    setModalStatus(
      t('import.success', {
        name: orbName,
        version: result.report.manifest.version,
        path: result.stored_zip_path,
        zip_sha256: result.report.zip_sha256,
        assets_sha256: result.report.assets_sha256,
      }),
      false
    );
    await refreshLibrary();
    showRestartHint();
    setTimeout(hideImportModal, 2000);
    return true;
  } catch (error) {
    setModalStatus(error, true);
    $('btn-modal-import').disabled = false;
    return false;
  }
}

async function confirmImport() {
  const path = importState.selectedPath;
  if (!path) return;
  setModalStatus(t('import.validating'), false);
  try {
    const inspect = await invoke('inspect_zip', { path });
    if (inspect.password_protected) {
      showPreImportPasswordDialog(path, inspect.manifest_name || inspect.manifest_version);
      return;
    }
    await doImport(path, null);
  } catch (error) {
    setModalStatus(error, true);
    $('btn-modal-import').disabled = false;
  }
}

function setModalStatus(message, isError) {
  const el = $('modal-import-status');
  el.textContent = message;
  if (message) {
    el.style.display = '';
    el.className = 'status-card' + (isError ? ' error' : '');
  } else {
    el.style.display = 'none';
  }
}

function normalizeFsPath(value) {
  return String(value || '').replace(/\\/g, '/');
}

let storeLoaded = false;

function showTab(name) {
  state.activeTab = name;
  document.querySelectorAll('.tab-item').forEach((button) => {
    button.classList.toggle('active', button.dataset.tab === name);
  });
  document.querySelectorAll('.tab-panel').forEach((panel) => {
    panel.classList.toggle('active', panel.id === `tab-${name}`);
  });
  
  if (name === 'store' && !storeLoaded) {
    storeLoaded = true;
    storeListTags().then(() => {
      storeSearch("", null, null, 1);
    });
  }
}

async function loadStatus() {
  if (!invoke) {
    $('app-version').textContent = t('status.static_preview');
    return;
  }
  try {
    const status = await invoke('runtime_status');
    $('app-version').textContent = `v${status.version}`;
  } catch (error) {
    console.error(error);
    $('app-version').textContent = t('status.static_preview');
  }
}

async function refreshLibrary() {
  if (!invoke) return renderLibrary([]);
  feedbackBtn($('btn-refresh-library'), 'feedback.refreshed');
  try {
    state.orbs = await invoke('list_orbs');
    state.orbSecurityById = {};
    state.libraryPage = 1;
    renderLibrary(state.orbs);
    await refreshOrbSecurityState(state.orbs);
    renderLibrary(state.orbs);
    syncOrbSelects();
  } catch (error) {
    $('library-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
}

async function checkLibraryHealth() {
  if (!invoke) return;
  const banner = $('library-bookmark-stale-banner');
  if (!banner) return;
  try {
    const health = await invoke('get_library_health');
    $('library-bookmark-stale-text').textContent = t('library.bookmark_stale_banner');
    banner.style.display = health.bookmark_stale ? '' : 'none';
  } catch (_) { /* best-effort */ }
}

async function fixLibraryBookmark() {
  if (!invoke) return;
  try {
    const result = await invoke('choose_orb_library_dir_suggested');
    if (!result?.path) return;
    if (result.pending) {
      state.pendingLibraryChange = { path: result.path, orbCount: result.orb_count };
      showLibraryChangeModal();
      return;
    }
    state.orbLibraryDir = result.path;
    const dirInput = $('settings-orb-library-dir');
    if (dirInput) dirInput.value = result.path;
    await checkLibraryHealth();
    await refreshLibrary();
  } catch (error) {
    const banner = $('library-bookmark-stale-banner');
    if (banner) banner.style.display = 'none';
    $('library-list').innerHTML = `<div class="status-card error">${escapeHtml(t('settings.orb_library_choose_error') + ' ' + error)}</div>`;
  }
}

async function refreshOrbSecurityState(orbs) {
  if (!invoke) return;
  const rememberable = (orbs || []).filter((orb) => orb.password_protected);
  if (!rememberable.length) return;

  const entries = await Promise.all(rememberable.map(async (orb) => {
    try {
      const security = await invoke('get_orb_security', { orbId: orb.id });
      return [orb.id, security];
    } catch {
      return [orb.id, null];
    }
  }));

  state.orbSecurityById = {
    ...state.orbSecurityById,
    ...Object.fromEntries(entries),
  };
}

function renderLibrary(orbs) {
  // Apply name/description filter if set
  const filterText = ($('filter-query').value || '').trim().toLowerCase();
  const filtered = filterText
    ? orbs.filter((orb) =>
        (orb.display_name || '').toLowerCase().includes(filterText) ||
        (orb.description || '').toLowerCase().includes(filterText)
      )
    : orbs;

  if (!filtered.length) {
    const msg = filterText
      ? `<div class="status-card muted-card">${t('library.no_match', { query: escapeHtml(filterText) })}</div>`
      : `<div class="status-card muted-card">${t('library.no_orbs')}</div>`;
    $('library-list').innerHTML = msg;
    $('library-pagination').innerHTML = '';
    return;
  }

  // Paginate filtered results
  const pageSize = state.libraryPageSize;
  const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));
  state.libraryTotalPages = totalPages;
  // Clamp current page
  if (state.libraryPage > totalPages) state.libraryPage = totalPages;
  const start = (state.libraryPage - 1) * pageSize;
  const pageOrbs = filtered.slice(start, start + pageSize);

  $('library-list').innerHTML = pageOrbs.map((orb) => {
    const passwordBadge = orb.password_protected
      ? `<span class="store-pill">🔒 ${escapeHtml(t('library.password_badge'))}${orb.password_persistence ? ` · ${escapeHtml(orb.password_persistence === 'remember_on_this_device' ? t('library.password_remembered') : t('library.password_every_launch'))}` : ''}</span>`
      : '';
    return `
    <article class="orb-card" data-testid="library-orb-card" data-orb-id="${escapeHtml(orb.id)}">
      <div>
        <div class="orb-title">${escapeHtml(orb.display_name)}</div>
        <div class="orb-meta">${escapeHtml(orb.install_source)} · ${orb.encrypted_assets ? 'encrypted' : 'plaintext'}${passwordBadge ? ` ${passwordBadge}` : ''}</div>
        <div class="orb-desc">${escapeHtml(orb.description || 'No description')}</div>
        <div class="orb-hash">zip ${escapeHtml(orb.zip_sha256)}<br>assets ${escapeHtml(orb.assets_sha256)}</div>
        <div class="orb-stats-row" id="stats-${escapeHtml(orb.id)}"><span class="muted">—</span></div>
      </div>
      <div style="display:flex;gap:8px;">
        <button class="btn btn-secondary" data-search-orb="${escapeHtml(orb.id)}" data-testid="orb-search-btn">${t('library.search_btn')}</button>
        <button class="btn btn-secondary" data-qa-orb="${escapeHtml(orb.id)}" data-testid="orb-qa-btn">${t('library.qa_btn')}</button>
        <button class="btn btn-danger" data-delete-orb="${escapeHtml(orb.id)}" data-testid="orb-delete-btn">${t('library.delete_btn')}</button>
      </div>
    </article>`;
  }).join('');
  document.querySelectorAll('[data-search-orb]').forEach((button) => {
    button.addEventListener('click', () => showOrbSearchModal(button.dataset.searchOrb));
  });
  document.querySelectorAll('[data-qa-orb]').forEach((button) => {
    button.addEventListener('click', () => showQaModal(button.dataset.qaOrb));
  });
  document.querySelectorAll('[data-delete-orb]').forEach((button) => {
    button.addEventListener('click', () => deleteOrb(button.dataset.deleteOrb));
  });
  pageOrbs.forEach((orb) => {
    fetchAndRenderStats(orb.id);
  });
  renderLibraryPagination(filtered.length);
  togglePlatformConfigsSection();
}

function renderLibraryPagination(totalCount) {
  const p = state.libraryPage;
  const total = state.libraryTotalPages;
  if (total <= 1) {
    $('library-pagination').innerHTML = '';
    return;
  }
  $('library-pagination').innerHTML = `
    <button class="btn btn-secondary library-page-btn" data-library-page="${p - 1}" ${p <= 1 ? 'disabled' : ''}>${t('qa.prev')}</button>
    <span class="library-page-info">${t('library.page_info', { page: p, total })}</span>
    <button class="btn btn-secondary library-page-btn" data-library-page="${p + 1}" ${p >= total ? 'disabled' : ''}>${t('qa.next')}</button>
  `;
  document.querySelectorAll('[data-library-page]').forEach((btn) => {
    btn.addEventListener('click', () => {
      if (btn.disabled) return;
      const newPage = parseInt(btn.dataset.libraryPage, 10);
      if (newPage >= 1 && newPage <= state.libraryTotalPages) {
        state.libraryPage = newPage;
        renderLibrary(state.orbs);
      }
    });
  });
}

async function fetchAndRenderStats(orbId) {
  if (!invoke) return;
  try {
    const metrics = await invoke('get_orb_metrics', { orbId });
    const statsEl = $(`stats-${orbId}`);
    if (!statsEl) return;
    statsEl.innerHTML = `
      <span class="orb-stat-item"><span class="stat-value">${metrics.total_requests}</span> <span class="stat-label">${t('library.stats_requests', { total: '' }).replace(/: $/, '')}</span></span>
      <span class="orb-stat-item"><span class="stat-value">${metrics.stdio_requests}</span> <span class="stat-badge stdio-badge">STDIO</span></span>
      <span class="orb-stat-item"><span class="stat-value">${metrics.http_requests}</span> <span class="stat-badge http-badge">HTTP</span></span>
    `;
  } catch (e) {
    // Orb might have stopped between render and fetch
    const statsEl = $(`stats-${orbId}`);
    if (statsEl) statsEl.innerHTML = `<span class="muted">${escapeHtml(String(e))}</span>`;
  }
}

function syncOrbSelects() {
  // No longer needed — gateway mode uses a single config without orb selection
}

// ── Restart hint banner (shown after import/delete) ──────────────────────────

function showRestartHint() {
  const hint = $('library-restart-hint');
  if (!hint) return;
  $('library-restart-hint-text').textContent = t('library.restart_hint');
  hint.style.display = 'flex';
}

function hideRestartHint() {
  const hint = $('library-restart-hint');
  if (hint) hint.style.display = 'none';
}

// ── Orb list filter (searches by name/description) ──────────────────────────

function filterLibrary() {
  feedbackBtn($('btn-filter-library'), 'feedback.filtered');
  state.libraryPage = 1;
  renderLibrary(state.orbs);
}

// ── Orb search modal ──────────────────────────────────────────────────────

function orbSearchModalVisible() {
  return $('orb-search-modal').style.display !== 'none';
}

function showOrbSearchModal(orbId) {
  state.orbSearchTargetId = orbId || null;
  const orb = state.orbs.find((o) => o.id === orbId);
  $('orb-search-title').textContent = orb
    ? `${t('orbsearch.title')} — ${orb.display_name}`
    : t('orbsearch.title');
  $('orb-search-query').value = '';
  $('orb-search-results').innerHTML = '';
  $('orb-search-results').scrollTop = 0;
  $('orb-search-status').textContent = '';
  $('orb-search-status').className = 'status-line';
  $('orb-search-pw-area').style.display = 'none';
  $('orb-search-pw-input').value = '';
  state.orbSearchPassword = null;
  $('orb-search-modal').style.display = '';
  $('orb-search-query').focus();
}

function hideOrbSearchModal() {
  $('orb-search-modal').style.display = 'none';
}

async function runOrbSearch() {
  const orbId = state.orbSearchTargetId;
  const query = $('orb-search-query').value.trim();
  if (!orbId || !query) {
    setOrbSearchStatus(t('orbsearch.enter_query'), true);
    return;
  }
  // Use stored password from previous prompt
  const password = state.orbSearchPassword || $('orb-search-pw-input').value || undefined;
  feedbackBtn($('btn-orb-search-go'), 'feedback.filtered');
  setOrbSearchStatus(t('orbsearch.searching'), false);
  $('orb-search-results').innerHTML = '';
  try {
    const response = await invoke('search_orb', {
      orbId,
      query,
      password: password || null,
      method: $('orb-search-method').value,
      topK: 50,
    });
    setOrbSearchStatus(`${response.hits.length} hit(s) · ${response.active_plan}`, false);
    renderOrbSearchResults(response.hits);
  } catch (error) {
    const msg = String(error);
    // If the orb is encrypted and needs a password, show inline prompt
    if (msg.includes('password required')) {
      $('orb-search-pw-area').style.display = 'flex';
      $('orb-search-pw-input').focus();
      setOrbSearchStatus(t('orbsearch.encrypted_prompt'), true);
    } else {
      setOrbSearchStatus(msg, true);
    }
  }
}

function submitOrbSearchPassword() {
  state.orbSearchPassword = $('orb-search-pw-input').value;
  $('orb-search-pw-area').style.display = 'none';
  runOrbSearch();
}

function setOrbSearchStatus(message, isError) {
  $('orb-search-status').textContent = message;
  $('orb-search-status').classList.toggle('error', Boolean(isError));
}

function renderOrbSearchResults(hits) {
  if (!hits.length) {
    $('orb-search-results').innerHTML = `<div class="status-card muted-card">${t('orbsearch.no_matches')}</div>`;
    return;
  }
  $('orb-search-results').innerHTML = hits.map((hit) => `
    <article class="result-item" data-testid="orb-search-hit">
      <div class="result-meta">${escapeHtml(hit.document_title)}${hit.page ? ` · p.${hit.page}` : ''} · ${escapeHtml(hit.method)} · ${Number(hit.score).toFixed(3)}</div>
      <div class="result-text">${escapeHtml(hit.text)}</div>
    </article>
  `).join('');
}

// ── Q&A History Modal ──────────────────────────────────────────────────────

function qaModalVisible() {
  return $('qa-modal').style.display !== 'none';
}

function showQaModal(orbId) {
  state.qaOrbId = orbId || null;
  state.qaPage = 1;
  const orb = state.orbs.find((o) => o.id === orbId);
  $('qa-modal-title').textContent = orb
    ? `${t('qa.title')} — ${orb.display_name}`
    : t('qa.title');
  $('qa-stats-summary').innerHTML = '';
  $('qa-list').innerHTML = '';
  $('qa-pagination').innerHTML = '';
  $('qa-status').textContent = '';
  $('qa-modal').style.display = '';
  loadQaModalData(orbId);
}

function hideQaModal() {
  $('qa-modal').style.display = 'none';
}


async function loadQaModalData(orbId) {
  $('qa-status').textContent = t('qa.loading');
  $('qa-status').classList.remove('error');
  try {
    // Fetch both metrics summary and Q&A history in parallel
    const [metrics, qaHistory] = await Promise.all([
      invoke('get_orb_metrics', { orbId }).catch(() => null),
      invoke('get_orb_qa_history', { orbId, page: state.qaPage, pageSize: 20 }).catch(() => null),
    ]);

    if (!metrics && !qaHistory) {
      $('qa-status').textContent = t('qa.not_running');
      $('qa-status').classList.add('error');
      return;
    }

    // Render stats summary
    if (metrics) {
      renderQaStatsSummary(metrics);
    }

    // Render Q&A entries
    if (qaHistory) {
      renderQaHistory(qaHistory);
    } else {
      $('qa-list').innerHTML = `<div class="status-card muted-card">${t('qa.no_history')}</div>`;
    }

    $('qa-status').textContent = '';
  } catch (error) {
    $('qa-status').textContent = `${t('qa.error')} ${escapeHtml(String(error))}`;
    $('qa-status').classList.add('error');
  }
}

function renderQaStatsSummary(metrics) {
  $('qa-stats-summary').innerHTML = `
    <div class="qa-stat-block">
      <span class="qa-stat-value">${metrics.total_requests}</span>
      <span class="qa-stat-label">${t('qa.total_requests')}</span>
    </div>
    <div class="qa-stat-block">
      <span class="qa-stat-value">${metrics.stdio_requests}</span>
      <span class="qa-stat-label">${t('qa.stdio_requests')}</span>
    </div>
    <div class="qa-stat-block">
      <span class="qa-stat-value">${metrics.http_requests}</span>
      <span class="qa-stat-label">${t('qa.http_requests')}</span>
    </div>
  `;
}

function renderQaHistory(response) {
  state.qaTotalPages = response.total_pages;

  if (!response.items.length) {
    $('qa-list').innerHTML = `<div class="status-card muted-card">${t('qa.no_history')}</div>`;
    $('qa-pagination').innerHTML = '';
    return;
  }

  $('qa-list').innerHTML = response.items.map((entry) => {
    const ts = formatTimestamp(entry.timestamp);
    const queryTruncated = entry.query.length > 120
      ? `<span class="query-truncated" title="${escapeHtml(entry.query)}">${escapeHtml(entry.query.slice(0, 120))}…</span>`
      : escapeHtml(entry.query);
    const respTruncated = entry.response_preview.length > 200
      ? `<span class="response-truncated" title="${escapeHtml(entry.response_preview)}">${escapeHtml(entry.response_preview.slice(0, 200))}…</span>`
      : escapeHtml(entry.response_preview || '—');

    const transportClass = entry.transport === 'http' ? 'http' : 'stdio';
    const transportLabel = entry.transport === 'http' ? 'HTTP' : 'STDIO';

    return `
      <article class="qa-entry" data-testid="qa-entry">
        <div class="qa-entry-header">
          <span>
            <span class="qa-entry-transport ${transportClass}">${transportLabel}</span>
            <span class="qa-entry-method">${escapeHtml(entry.method)}</span>
            <span>${t('qa.results', { count: entry.num_results })}</span>
          </span>
          <span class="qa-entry-timestamp">${escapeHtml(ts)}</span>
        </div>
        <div class="qa-entry-query"><strong>${t('qa.query')}:</strong> ${queryTruncated}</div>
        <div class="qa-entry-response"><strong>${t('qa.response')}:</strong> ${respTruncated}</div>
      </article>`;
  }).join('');

  renderQaPagination();
}

function renderQaPagination() {
  const p = state.qaPage;
  const total = state.qaTotalPages;
  if (total <= 1) {
    $('qa-pagination').innerHTML = '';
    return;
  }
  $('qa-pagination').innerHTML = `
    <button class="btn btn-secondary qa-page-btn" data-qa-page="${p - 1}" ${p <= 1 ? 'disabled' : ''}>${t('qa.prev')}</button>
    <span class="qa-page-info">${t('qa.page', { page: p, total })}</span>
    <button class="btn btn-secondary qa-page-btn" data-qa-page="${p + 1}" ${p >= total ? 'disabled' : ''}>${t('qa.next')}</button>
  `;
  document.querySelectorAll('[data-qa-page]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const newPage = parseInt(btn.dataset.qaPage, 10);
      if (newPage >= 1 && newPage <= state.qaTotalPages && state.qaOrbId) {
        state.qaPage = newPage;
        loadQaModalData(state.qaOrbId);
      }
    });
  });
}

function formatTimestamp(isoStr) {
  if (!isoStr) return '—';
  let d;
  if (isoStr.includes('T')) {
    d = new Date(isoStr);
  } else if (/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}/.test(isoStr)) {
    d = new Date(isoStr + 'Z');
  } else {
    const secs = parseInt(isoStr, 10);
    if (!isNaN(secs) && secs > 1000000000) {
      d = new Date(secs * 1000);
    }
  }
  if (d && !isNaN(d.getTime())) {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    const hh = String(d.getHours()).padStart(2, '0');
    const mm = String(d.getMinutes()).padStart(2, '0');
    const ss = String(d.getSeconds()).padStart(2, '0');
    return `${y}-${m}-${day} ${hh}:${mm}:${ss}`;
  }
  return isoStr;
}

async function generateMcpConfig() {
  feedbackBtn($('btn-generate-config'), 'feedback.generated');
  try {
    const snippets = await invoke('gateway_mcp_config_snippets');
  $('mcp-config-list').innerHTML = snippets.map((snippet) => `
    <article class="config-card" data-testid="mcp-config-card">
      <div class="config-card-header">
          <div class="config-meta">${escapeHtml(snippet.label)}</div>
          <button class="btn btn-secondary btn-sm" data-copy-config="${escapeHtml(snippet.client)}">${t('mcp.copy_config_btn')}</button>
        </div>
        <textarea readonly id="config-json-${escapeHtml(snippet.client)}">${escapeHtml(snippet.json)}</textarea>
      </article>
    `).join('');
    document.querySelectorAll('[data-copy-config]').forEach((btn) => {
      btn.addEventListener('click', () => copyStdioConfig(btn.dataset.copyConfig, btn));
    });
  } catch (error) {
    $('mcp-config-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
}

async function copyStdioConfig(client, btn) {
  const textarea = document.getElementById(`config-json-${client}`);
  if (!textarea) return;
  try {
    await navigator.clipboard.writeText(textarea.value);
  } catch {
    // Fallback for non-HTTPS environments
    textarea.select();
    document.execCommand('copy');
  }
  feedbackBtn(btn, 'feedback.copied');
}

function escapeHtml(value) {
  return String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

async function loadSettings() {
  if (!invoke) return;
  try {
    const settings = await invoke('get_settings');
    $('settings-http-port').value = settings.http_port || 5599;
    $('settings-network-binding').value = settings.network_binding || 'localhost';
    try {
      state.platform = await invoke('get_platform');
    } catch (_) {
      state.platform = 'unknown';
    }
    const libraryGroup = $('orb-library-group');
    if (libraryGroup) libraryGroup.style.display = isMacPlatform(state.platform) ? '' : 'none';
    if (settings.orb_library_dir) {
      state.orbLibraryDir = settings.orb_library_dir;
      $('settings-orb-library-dir').value = settings.orb_library_dir;
    }
    // Show first-launch onboarding on macOS when user hasn't set a folder yet.
    if (isMacPlatform(state.platform) && !settings.onboarding_complete && !settings.orb_library_dir) {
      showOnboardingModal();
    }
    // Warn if the saved bookmark is stale (e.g. after a TestFlight reinstall).
    if (isMacPlatform(state.platform) && settings.orb_library_dir) {
      try {
        const health = await invoke('get_library_health');
        const statusEl = $('settings-status');
        if (health.bookmark_stale && statusEl) {
          statusEl.textContent = t('settings.orb_library_bookmark_stale');
          statusEl.classList.add('error');
        }
      } catch (_) {}
    }
    captureSettingsSnapshot();
    updateUnsavedHint();
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
}

async function chooseOrbLibraryDir() {
  if (!invoke) return;
  try {
    const result = await invoke('choose_orb_library_dir_suggested');
    if (!result?.path) return;
    if (result.pending) {
      state.pendingLibraryChange = { path: result.path, orbCount: result.orb_count };
      showLibraryChangeModal();
      return;
    }
    state.orbLibraryDir = result.path;
    $('settings-orb-library-dir').value = result.path;
    feedbackBtn($('btn-choose-orb-library'), 'settings.orb_library_changed');
    $('settings-status').textContent = t('settings.orb_library_changed');
    $('settings-status').classList.remove('error');
    markSettingsEdited();
  } catch (error) {
    $('settings-status').textContent = t('settings.orb_library_choose_error') + ' ' + error;
    $('settings-status').classList.add('error');
  }
}

// ── Orb library folder change (migrate / delete old orbs) ──────────────────

function showLibraryChangeModal() {
  const pending = state.pendingLibraryChange;
  if (!pending) return;
  $('library-change-message').textContent = t('librarychange.message', { count: pending.orbCount });
  $('library-change-status').textContent = '';
  $('library-change-status').className = 'status-line';
  $('library-change-modal').style.display = 'flex';
}

function hideLibraryChangeModal() {
  $('library-change-modal').style.display = 'none';
  $('library-change-status').textContent = '';
  state.pendingLibraryDelete = false;
}

async function cancelLibraryChange() {
  try {
    await invoke('cancel_orb_library_change');
  } catch (_) { /* best-effort */ }
  state.pendingLibraryChange = null;
  const fromOnboarding = state.fromOnboarding;
  state.fromOnboarding = false;
  hideLibraryChangeModal();
  if (fromOnboarding) showOnboardingModal();
}

async function migrateLibraryChange() {
  if (!invoke) return;
  $('btn-library-change-migrate').disabled = true;
  $('btn-library-change-delete').disabled = true;
  try {
    const result = await invoke('apply_orb_library_change', { action: 'migrate' });
    applyOrbLibraryDir(result.path);
    captureSettingsSnapshot();
    updateUnsavedHint();
    hideLibraryChangeModal();
    state.fromOnboarding = false;
    state.pendingLibraryChange = null;
    $('settings-status').textContent = t('librarychange.migrated', { count: result.orb_count });
    $('settings-status').classList.remove('error');
  } catch (error) {
    $('library-change-status').textContent = t('librarychange.error', { error });
    $('library-change-status').classList.add('error');
  } finally {
    $('btn-library-change-migrate').disabled = false;
    $('btn-library-change-delete').disabled = false;
  }
}

function deleteLibraryChange() {
  const pending = state.pendingLibraryChange;
  if (!pending) return;
  // Second confirmation via the existing danger modal; the library-change
  // modal stays open underneath so the user can still pick Migrate.
  state.pendingLibraryDelete = true;
  $('confirm-modal-message').textContent = t('librarychange.delete_confirm', { count: pending.orbCount });
  $('confirm-modal').style.display = '';
}

async function doDeleteLibraryChange() {
  if (!invoke) return;
  try {
    const result = await invoke('apply_orb_library_change', { action: 'delete' });
    hideConfirmDeleteModal();
    applyOrbLibraryDir(result.path);
    captureSettingsSnapshot();
    updateUnsavedHint();
    await refreshLibrary();
    syncOrbSelects();
    refreshRunning();
    hideLibraryChangeModal();
    state.fromOnboarding = false;
    state.pendingLibraryChange = null;
    $('settings-status').textContent = t('librarychange.deleted', { count: result.orb_count });
    $('settings-status').classList.remove('error');
  } catch (error) {
    hideConfirmDeleteModal();
    $('library-change-status').textContent = t('librarychange.error', { error });
    $('library-change-status').classList.add('error');
  }
}

// --- Onboarding modal (macOS first-launch) ---

function showOnboardingModal() {
  $('onboarding-modal').style.display = 'flex';
  $('onboarding-status').textContent = '';
}

function hideOnboardingModal() {
  $('onboarding-modal').style.display = 'none';
}

function applyOrbLibraryDir(dir) {
  if (!dir) return;
  state.orbLibraryDir = dir;
  $('settings-orb-library-dir').value = dir;
  markSettingsEdited();
}

async function onboardingUseDefault() {
  if (!invoke) return;
  $('onboarding-status').textContent = '';
  $('btn-onboarding-default').disabled = true;
  $('btn-onboarding-choose').disabled = true;
  try {
    const result = await invoke('choose_orb_library_dir_suggested');
    if (!result?.path) return;
    if (result.pending) {
      state.pendingLibraryChange = { path: result.path, orbCount: result.orb_count };
      state.fromOnboarding = true;
      // Close the onboarding modal first: both overlays share z-index 1000 and
      // the onboarding modal sits later in the DOM, so it would otherwise cover
      // the migrate/delete confirmation and leave the user stuck on Skip.
      hideOnboardingModal();
      showLibraryChangeModal();
      return;
    }
    applyOrbLibraryDir(result.path);
    hideOnboardingModal();
  } catch (error) {
    $('onboarding-status').textContent = t('onboarding.error', { error });
    $('onboarding-status').classList.add('error');
  } finally {
    $('btn-onboarding-default').disabled = false;
    $('btn-onboarding-choose').disabled = false;
  }
}

async function onboardingChooseDifferent() {
  if (!invoke) return;
  $('onboarding-status').textContent = '';
  $('btn-onboarding-default').disabled = true;
  $('btn-onboarding-choose').disabled = true;
  try {
    const result = await invoke('choose_orb_library_dir');
    if (!result?.path) return;
    if (result.pending) {
      state.pendingLibraryChange = { path: result.path, orbCount: result.orb_count };
      state.fromOnboarding = true;
      hideOnboardingModal();
      showLibraryChangeModal();
      return;
    }
    applyOrbLibraryDir(result.path);
    hideOnboardingModal();
  } catch (error) {
    $('onboarding-status').textContent = t('onboarding.error', { error });
    $('onboarding-status').classList.add('error');
  } finally {
    $('btn-onboarding-default').disabled = false;
    $('btn-onboarding-choose').disabled = false;
  }
}

async function onboardingSkip() {
  if (!invoke) return;
  try {
    await invoke('dismiss_onboarding');
  } catch (_) { /* best-effort */ }
  hideOnboardingModal();
}

// --- End onboarding ---

// --- Unsaved-settings detection ---

let savedSettingsSnapshot = null;

function captureSettingsSnapshot() {
  savedSettingsSnapshot = {
    http_port: $('settings-http-port').value,
    network_binding: $('settings-network-binding').value,
    orb_library_dir: state.orbLibraryDir || '',
  };
}

function settingsHaveUnsavedChanges() {
  if (!savedSettingsSnapshot) return false;
  return (
    savedSettingsSnapshot.http_port !== $('settings-http-port').value ||
    savedSettingsSnapshot.network_binding !== $('settings-network-binding').value ||
    savedSettingsSnapshot.orb_library_dir !== (state.orbLibraryDir || '')
  );
}

function updateUnsavedHint() {
  const status = $('settings-status');
  if (!status) return;
  const saveBtn = $('btn-save-settings');
  if (settingsHaveUnsavedChanges()) {
    status.textContent = t('settings.unsaved_hint');
    status.classList.remove('error');
    if (saveBtn) saveBtn.classList.add('btn-dirty');
  } else if (saveBtn) {
    // Keep any existing status text (e.g. "Settings saved.") intact.
    saveBtn.classList.remove('btn-dirty');
  }
}

function markSettingsEdited() {
  updateUnsavedHint();
}

// --- End unsaved-settings detection ---

async function saveSettings() {
  if (!invoke) return;
  feedbackBtn($('btn-save-settings'), 'feedback.saved');
  const settings = {
    http_port: parseInt($('settings-http-port').value, 10) || 5599,
    network_binding: $('settings-network-binding').value,
    orb_library_dir: state.orbLibraryDir || null,
  };
  try {
    await invoke('save_settings', { settings });
    $('settings-status').textContent = t('settings.saved');
    $('settings-status').classList.remove('error');
    captureSettingsSnapshot();
    updateUnsavedHint();
    // Refresh the HTTP tab so the gateway URL / config JSON reflect the
    // new network binding (the gateway process restarts on change).
    refreshRunning();
  } catch (error) {
    $('settings-status').textContent = String(error);
    $('settings-status').classList.add('error');
  }
}

async function refreshRunning() {
  if (!invoke) return renderRunning([]);
  feedbackBtn($('btn-refresh-running'), 'feedback.refreshed');
  try {
    const running = await invoke('list_running_orbs');
    state.runningOrbIds = running.map((r) => r.orb_id);
    renderRunning(running);
    if (state.orbs.length) renderLibrary(state.orbs);
  } catch (error) {
    $('running-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
}

function renderRunning(running) {
  if (!state.orbs.length) {
    $('running-list').innerHTML = `<div class="status-card muted-card">${t('running.no_orbs')}</div>`;
    return;
  }
  // Show gateway HTTP config
  $('running-list').innerHTML = `
    <article class="config-card" data-testid="gateway-card">
      <div class="config-card-header">
        <div class="config-meta">MCPOrb Gateway HTTP</div>
        <button class="btn btn-secondary btn-sm" id="copy-gateway-http-config">${t('running.copy_config_btn')}</button>
      </div>
      <div id="gateway-status-line" class="status-line muted-card">${t('running.loading')}</div>
      <div id="gateway-note" class="note-card hidden"></div>
      <button class="btn btn-primary btn-sm hidden" id="btn-gateway-toggle"></button>
      <div id="gateway-conn-row" class="hidden">
        <div class="conn-string-title">${t('running.gateway_conn_title')}</div>
        <div class="conn-string-wrap">
          <input readonly id="gateway-conn-string" class="conn-string-input" spellcheck="false">
          <button class="btn btn-secondary btn-sm" id="copy-gateway-conn">${t('running.gateway_copy_conn')}</button>
          <button class="btn btn-secondary btn-sm" id="reset-gateway-token">${t('running.gateway_reset_token')}</button>
        </div>
      </div>
      <textarea readonly id="gateway-http-config-json">${t('running.loading')}</textarea>
    </article>
  `;
  refreshGatewayStatus();
  refreshGatewayHttpConfig();
}

async function refreshGatewayHttpConfig() {
  if (!invoke) return;
  const area = $('gateway-http-config-json');
  if (!area) return;
  try {
    const snippets = await invoke('gateway_http_config_snippets');
    area.value = snippets.length > 0
      ? snippets[0].json
      : '/* Gateway config unavailable */';
    const copyBtn = $('copy-gateway-http-config');
    if (copyBtn) {
      copyBtn.onclick = async () => {
        if (snippets.length > 0) {
          await navigator.clipboard.writeText(snippets[0].json);
          feedbackBtn(copyBtn, 'feedback.copied');
        }
      };
    }
  } catch (error) {
    area.value = `/* Error: ${error} */`;
  }
}

async function refreshGatewayStatus() {
  if (!invoke) return;
  const line = $('gateway-status-line');
  const btn = $('btn-gateway-toggle');
  if (!line || !btn) return;
  try {
    const s = await invoke('unified_gateway_status');
    const noteEl = $('gateway-note');
    if (noteEl) {
      if (s.note) {
        noteEl.textContent = s.note;
        noteEl.classList.remove('hidden');
      } else {
        noteEl.classList.add('hidden');
      }
    }
    const connRow = $('gateway-conn-row');
    if (s.running) {
      line.textContent = t('running.gateway_status_running', { url: s.url });
      btn.textContent = t('running.gateway_stop_btn');
      btn.className = 'btn btn-secondary btn-sm';
      btn.onclick = async () => {
        btn.textContent = t('running.gateway_stopping');
        btn.disabled = true;
        try {
          await invoke('stop_unified_gateway');
        } catch (error) {
          line.textContent = t('running.gateway_stop_failed', { error: String(error) });
        }
        btn.disabled = false;
        refreshGatewayStatus();
      };
      if (connRow) {
        const resetBtn = $('reset-gateway-token');
        if (resetBtn) {
          resetBtn.textContent = t('running.gateway_reset_token');
          resetBtn.disabled = false;
        }
        const conn = s.token ? `${s.url}?token=${encodeURIComponent(s.token)}` : s.url;
        connRow.classList.remove('hidden');
        const input = $('gateway-conn-string');
        if (input) input.value = conn;
        const copyBtn = $('copy-gateway-conn');
        if (copyBtn) copyBtn.onclick = async () => {
          await navigator.clipboard.writeText(conn);
          feedbackBtn(copyBtn, 'running.gateway_token_copied');
        };
        if (resetBtn) resetBtn.onclick = async () => {
          let confirmed = false;
          try {
            // Direct IPC: the dialog plugin overrides window.confirm with a
            // call to a nonexistent `plugin:dialog|confirm` command, so use
            // the real `message` command instead.
            const result = await invoke('plugin:dialog|message', {
              message: t('running.gateway_reset_confirm'),
              title: t('running.gateway_reset_token'),
              kind: 'warning',
              buttons: 'OkCancel',
            });
            confirmed = result === 'Ok';
          } catch (error) {
            confirmed = window.confirm(t('running.gateway_reset_confirm'));
          }
          if (!confirmed) return;
          resetBtn.textContent = t('running.gateway_resetting');
          resetBtn.disabled = true;
          try {
            await invoke('reset_gateway_token');
          } catch (err) {
            console.error('reset_gateway_token failed:', err);
            alert(t('running.gateway_reset_failed', { error: String(err) }));
          } finally {
            refreshGatewayStatus();
            refreshGatewayHttpConfig();
          }
        };
      }
    } else {
      line.textContent = t('running.gateway_status_stopped');
      btn.textContent = t('running.gateway_start_btn');
      btn.className = 'btn btn-primary btn-sm';
      btn.onclick = async () => {
        btn.textContent = t('running.gateway_starting');
        btn.disabled = true;
        try {
          await invoke('ensure_unified_gateway');
        } catch (error) {
          line.textContent = t('running.gateway_start_failed', { error: String(error) });
        }
        btn.disabled = false;
        refreshGatewayStatus();
      };
      if (connRow) connRow.classList.add('hidden');
    }
    btn.style.display = '';
  } catch (error) {
    line.textContent = `Error: ${escapeHtml(error)}`;
    btn.style.display = 'none';
  }
}

// ── Platform Config Discovery ──────────────────────────────────────────────

async function discoverPlatformConfigs() {
  if (!invoke) return;
  feedbackBtn($('btn-discover-configs'), 'feedback.refreshed');
  setPlatformConfigStatus(t('mcp.discovering'), false);
  try {
    state.platformConfigs = await invoke('discover_platform_configs');
    renderPlatformConfigs(state.platformConfigs);
  } catch (error) {
    setPlatformConfigStatus(error, true);
  }
}

function renderPlatformConfigs(configs) {
  if (!configs || !configs.length) {
    $('platform-config-list').innerHTML = `<div class="status-card muted-card">${t('mcp.no_configs')}</div>`;
    setPlatformConfigStatus('', false);
    return;
  }
  setPlatformConfigStatus('', false);
  $('platform-config-list').innerHTML = configs.map((cfg) => {
    const badgeClass = cfg.read_error ? 'badge-error'
      : cfg.exists ? 'badge-found'
      : 'badge-not-found';
    const badgeText = cfg.read_error ? t('mcp.config_read_error')
      : cfg.exists ? t('mcp.config_found')
      : t('mcp.config_not_found');
    const currentContent = cfg.current_content || '';
    const generatedContent = cfg.generated_content || '';
    const isSame = currentContent.trim() === generatedContent.trim();

    return `
      <article class="platform-config-card" data-testid="platform-config-card" data-platform="${escapeHtml(cfg.platform)}">
        <div class="platform-config-header">
          <div>
            <div class="platform-config-name">${escapeHtml(cfg.display_name)}</div>
            <div class="platform-config-path">${escapeHtml(cfg.location_label)}</div>
          </div>
          <span class="platform-config-badge ${badgeClass}">${badgeText}</span>
        </div>
        <div class="platform-config-editor">
          <div>
            <div style="font-size:11px;color:var(--color-text-dim);margin-bottom:4px;font-weight:700;">${t('mcp.current_label')}</div>
            <textarea readonly class="current-config" spellcheck="false">${escapeHtml(currentContent || '/* ' + t('mcp.config_not_found') + ' */')}</textarea>
          </div>
          <div>
            <div style="font-size:11px;color:var(--color-text-dim);margin-bottom:4px;font-weight:700;">${t('mcp.generated_label')}</div>
            <textarea readonly class="generated-config" spellcheck="false">${escapeHtml(generatedContent)}</textarea>
          </div>
        </div>
        <div class="platform-config-actions">
          <button class="btn btn-primary" data-apply-config="${escapeHtml(cfg.config_path)}" data-platform="${escapeHtml(cfg.platform)}" data-restart-hint-key="${escapeHtml(cfg.restart_hint || '')}" data-testid="apply-config-btn" ${!cfg.exists || isSame || !generatedContent ? 'disabled' : ''}>${t('mcp.apply_btn')}</button>
          ${cfg.restart_hint ? `<span class="platform-config-restart-hint">${escapeHtml(t(cfg.restart_hint))}</span>` : ''}
        </div>
        <div id="apply-status-${escapeHtml(cfg.platform)}" class="status-line"></div>
      </article>
    `;
  }).join('');

  // Wire Apply buttons
  document.querySelectorAll('[data-apply-config]').forEach((btn) => {
    btn.addEventListener('click', () => {
      const configPath = btn.dataset.applyConfig;
      const platform = btn.dataset.platform;
      const restartHintKey = btn.dataset.restartHintKey;
      const restartHint = restartHintKey ? t(restartHintKey) : '';
      const card = btn.closest('.platform-config-card');
      const textareas = card.querySelectorAll('textarea');
      const newContent = textareas.length >= 2 ? textareas[1].value : '';
      applyPlatformConfig(configPath, newContent, platform, restartHint, card);
    });
  });
}

async function applyPlatformConfig(configPath, newContent, platform, restartHint, card) {
  if (!invoke) return;
  const statusEl = card.querySelector('.status-line') || card.querySelector('[id^="apply-status-"]');
  if (statusEl) statusEl.textContent = t('mcp.applying');
  const applyBtn = card.querySelector('[data-apply-config]');
  if (applyBtn) applyBtn.disabled = true;
  try {
    const result = await invoke('apply_platform_config', {
      configPath,
      newContent,
      platform,
      restartHint,
    });
    if (result.success) {
      let msg = t('mcp.apply_success', { path: result.config_path });
      if (result.backup_path) {
        msg += ' · ' + t('mcp.apply_success_backup', { backup: result.backup_path });
      }
      if (statusEl) {
        statusEl.textContent = msg;
        statusEl.className = 'status-line';
      }
      feedbackBtn(applyBtn, 'feedback.saved');
    } else {
      if (statusEl) {
        statusEl.textContent = result.error || 'Unknown error';
        statusEl.className = 'status-line error';
      }
      if (applyBtn) applyBtn.disabled = false;
    }
  } catch (error) {
    if (statusEl) {
      statusEl.textContent = error;
      statusEl.className = 'status-line error';
    }
    if (applyBtn) applyBtn.disabled = false;
  }
}

function togglePlatformConfigsSection() {
  const hasOrbs = state.orbs && state.orbs.length > 0;
  const divider = $('mcp-section-divider');
  const section = $('mcp-platform-configs-section');
  if (divider) divider.style.display = hasOrbs ? '' : 'none';
  if (section) section.style.display = hasOrbs ? '' : 'none';
}

function setPlatformConfigStatus(message, isError) {
  const el = $('platform-config-status');
  if (el) {
    el.textContent = message;
    el.className = 'status-line' + (isError ? ' error' : '');
  }
}

async function stopOrbHttp(orbId) {
  if (!invoke) return;
  try {
    await invoke('stop_orb_http', { orbId });
    await refreshRunning();
  } catch (error) {
    alert(`Failed to stop Orb: ${error}`);
  }
}

window.stopOrbHttp = stopOrbHttp;

function deleteOrb(orbId) {
  state.pendingDeleteOrbId = orbId;
  const orb = state.orbs.find((o) => o.id === orbId);
  const name = orb ? orb.display_name : orbId;
  $('confirm-modal-message').textContent = t('library.delete_confirm', { name });
  $('confirm-modal').style.display = '';
}

function hideConfirmDeleteModal() {
  state.pendingDeleteOrbId = null;
  state.pendingLibraryDelete = false;
  $('confirm-modal').style.display = 'none';
}

function confirmModalVisible() {
  return $('confirm-modal').style.display !== 'none';
}

async function confirmDeleteOrb() {
  if (state.pendingLibraryDelete) {
    state.pendingLibraryDelete = false;
    return doDeleteLibraryChange();
  }
  const orbId = state.pendingDeleteOrbId;
  if (!orbId || !invoke) return;
  try {
    await invoke('remove_orb', { orbId });
    state.orbs = state.orbs.filter((o) => o.id !== orbId);
    renderLibrary(state.orbs);
    showRestartHint();
    syncOrbSelects();
    refreshRunning();
    hideConfirmDeleteModal();
  } catch (error) {
    alert(`Failed to delete Orb: ${error}`);
    hideConfirmDeleteModal();
  }
}

window.deleteOrb = deleteOrb;

// ── Import password dialog ──────────────────────────────────────────────────

function showPreImportPasswordDialog(path, orbName) {
  if (!invoke) return;
  state.pendingImportPath = path;
  $('import-password-desc').textContent = t('import.password_desc', { name: orbName || path });
  $('btn-import-password-submit').disabled = false;
  $('btn-import-password-cancel').disabled = false;
  $('import-password-input').value = '';
  $('import-password-input').disabled = false;
  $('import-password-status').textContent = '';
  $('import-password-status').className = 'status-line import-password-status';
  $('import-password-dialog').style.display = 'flex';
  setTimeout(() => $('import-password-input').focus(), 100);
}

function hideImportPasswordDialog() {
  $('import-password-dialog').style.display = 'none';
  state.pendingImportPath = null;
}

async function submitImportPassword() {
  const path = state.pendingImportPath;
  const password = $('import-password-input').value;
  if (!path || !password) return;

  $('btn-import-password-submit').disabled = true;
  $('btn-import-password-cancel').disabled = true;
  $('import-password-input').disabled = true;
  $('import-password-status').textContent = t('import.password_verifying');
  $('import-password-status').classList.remove('error');

  const ok = await doImport(path, password);
  if (ok) {
    // On success, update store button to "Imported ✓" if this was a store import
    if (state.pendingStoreArtifactId) {
      const btn = getImportBtn(state.pendingStoreArtifactId);
      setImportBtnState(btn, 'store.import_btn_done', false);
      state.pendingStoreArtifactId = null;
    }
    hideImportPasswordDialog();
  } else {
    // Wrong password / import error — keep dialog open for retry
    if (state.pendingStoreArtifactId) {
      const btn = getImportBtn(state.pendingStoreArtifactId);
      setImportBtnState(btn, 'store.import_btn', false);
    }
    $('btn-import-password-submit').disabled = false;
    $('btn-import-password-cancel').disabled = false;
    $('import-password-input').disabled = false;
    $('import-password-input').value = '';
    $('import-password-input').focus();
    $('import-password-status').textContent = t('import.password_incorrect') || 'Incorrect password. Please try again.';
    $('import-password-status').classList.add('error');
  }
}

function cancelImportPassword() {
  // Reset stuck store import button before closing
  if (state.pendingStoreArtifactId) {
    const btn = getImportBtn(state.pendingStoreArtifactId);
    setImportBtnState(btn, 'store.import_btn', false);
    state.pendingStoreArtifactId = null;
  }
  hideImportPasswordDialog();
}

async function copyHttpConfig() {
  if (!invoke) return;
  try {
    const snippets = await invoke('gateway_http_config_snippets');
    if (snippets.length > 0) {
      await navigator.clipboard.writeText(snippets[0].json);
      alert('HTTP MCP config copied to clipboard.');
    }
  } catch (error) {
    alert(`Failed to copy config: ${error}`);
  }
}

window.copyHttpConfig = copyHttpConfig;

let storeTagsLoaded = false;
let storeSelectedTag = null; // null = "All tags"
let storeTags = [];

function renderTagFilterList(tags, filterText) {
  const list = $('store-tag-filter-list');
  if (!list) return;

  const lowerFilter = (filterText || '').toLowerCase().trim();
  let filtered = tags;
  if (lowerFilter) {
    filtered = tags.filter(t => t.name.toLowerCase().includes(lowerFilter));
  }

  if (!filtered.length) {
    list.innerHTML = `<div class="tag-filter-empty">No matching tags</div>`;
    return;
  }

  list.innerHTML = filtered.map(t => `
    <div class="tag-filter-item${storeSelectedTag === t.slug ? ' selected' : ''}" data-tag-slug="${escapeHtml(t.slug)}">
      <span class="tag-filter-item-name">${escapeHtml(t.name)}</span>
      <span class="tag-filter-item-count">${t.count}</span>
    </div>
  `).join('');
}

function setStoreTag(slug) {
  const label = $('store-tag-filter-label');
  if (!slug) {
    storeSelectedTag = null;
    label.textContent = t('store.tag_all');
  } else {
    storeSelectedTag = slug;
    const tag = storeTags.find(t => t.slug === slug);
    label.textContent = tag ? `${tag.name} (${tag.count})` : slug;
  }
  renderTagFilterList(storeTags, $('store-tag-filter-input')?.value || '');
}

function toggleTagDropdown(show) {
  const dropdown = $('store-tag-filter-dropdown');
  const arrow = document.querySelector('.tag-filter-arrow');
  if (!dropdown) return;
  const visible = show !== undefined ? show : dropdown.style.display === 'none';
  dropdown.style.display = visible ? 'flex' : 'none';
  if (arrow) arrow.classList.toggle('open', visible);
  if (visible) {
    const input = $('store-tag-filter-input');
    if (input) {
      input.value = '';
      input.focus();
    }
    renderTagFilterList(storeTags, '');
  }
}

function onTagFilterItemClick(slug) {
  if (storeSelectedTag === slug) {
    setStoreTag(null); // deselect = "All tags"
  } else {
    setStoreTag(slug);
  }
  toggleTagDropdown(false);
  storeSearch();
}

async function storeListTags() {
  if (storeTagsLoaded || !invoke) return;
  try {
    storeTags = await invoke('store_list_tags');
    setStoreTag(null);
    storeTagsLoaded = true;
  } catch (error) {
    console.error('Failed to load store tags:', error);
  }
}

async function storeSearch(queryOverride, tagOverride, methodOverride, pageOverride) {
  if (queryOverride instanceof Event) {
    queryOverride = undefined;
  }
  const query = queryOverride !== undefined ? queryOverride : $('store-search-query').value.trim();
  const tag = tagOverride !== undefined ? tagOverride : storeSelectedTag;
  const method = methodOverride !== undefined ? methodOverride : ($('store-method-filter').value || null);
  const page = pageOverride !== undefined ? pageOverride : 1;

  state.storeSearchState = { query, tag, method, page };

  if (queryOverride !== undefined) $('store-search-query').value = query;
  if (tagOverride !== undefined) setStoreTag(tag || null);
  if (methodOverride !== undefined) $('store-method-filter').value = method || '';

  setStoreSearchStatus(`${t('orbsearch.searching')}...`, false);
  $('store-search-results').innerHTML = '';
  $('store-pagination').innerHTML = '';
  
  try {
    const result = await invoke('store_search', { query, tag, method, page });
    setStoreSearchStatus(t('store.results', { count: result.items.length }), false);
    renderStoreResults(result);
  } catch (error) {
    setStoreSearchStatus(String(error), true);
  }
}

function setStoreSearchStatus(message, isError) {
  $('store-search-status').textContent = message;
  $('store-search-status').classList.toggle('error', Boolean(isError));
}

function renderStoreResults(response) {
  const orbs = response.items;
  if (!orbs || !orbs.length) {
    $('store-search-results').innerHTML = `<div class="status-card muted-card">${t('store.no_results')}</div>`;
    $('store-pagination').innerHTML = '';
    return;
  }
  $('store-search-results').innerHTML = orbs.map((orb) => {
    const methodsHtml = (orb.methods || []).map(m => `<span class="store-pill">${escapeHtml(m)}</span>`).join('');
    const pwdStatus = orb.password_status === 'all' ? t('store.password_status_all') :
                      orb.password_status === 'partial' ? t('store.password_status_partial') :
                      t('store.password_status_none');
    return `
    <article class="orb-card" style="cursor:pointer;" data-testid="store-orb-card" data-slug="${escapeHtml(orb.slug)}" onclick="showStoreDetail('${escapeHtml(orb.slug)}')">
      <div>
        <h3 class="orb-title" style="margin:0 0 4px 0;">${escapeHtml(orb.display_name || orb.slug)}</h3>
        <div class="orb-meta" style="margin-bottom:8px;">
          <span class="store-pill">${escapeHtml(orb.version)}</span>
          <span class="store-pill">Pwd: ${pwdStatus}</span>
          ${methodsHtml}
        </div>
        <p class="orb-desc" style="margin:0;display:-webkit-box;-webkit-line-clamp:2;-webkit-box-orient:vertical;overflow:hidden;">${escapeHtml(orb.description || 'No description')}</p>
      </div>
    </article>
  `}).join('');
  
  renderStorePagination(response);
}

function renderStorePagination(response) {
  const p = response.page;
  const hasMore = response.has_more;
  
  if (p <= 1 && !hasMore) {
    $('store-pagination').innerHTML = '';
    return;
  }
  
  $('store-pagination').innerHTML = `
    <button class="btn btn-secondary store-page-btn" data-store-page="${p - 1}" ${p <= 1 ? 'disabled' : ''}>${t('qa.prev')}</button>
    <span class="store-page-info">Page ${p}</span>
    <button class="btn btn-secondary store-page-btn" data-store-page="${p + 1}" ${!hasMore ? 'disabled' : ''}>${t('qa.next')}</button>
  `;
  
  document.querySelectorAll('[data-store-page]').forEach((btn) => {
    btn.addEventListener('click', () => {
      if (btn.disabled) return;
      const newPage = parseInt(btn.dataset.storePage, 10);
      storeSearch(undefined, undefined, undefined, newPage);
    });
  });
}

// Deprecated — use storeDownloadArtifact instead
async function storeDownloadOrb(slug, hasPassword) {
  console.warn('storeDownloadOrb is deprecated, use storeDownloadArtifact');
  storeDownloadArtifact(slug, hasPassword);
}

window.storeDownloadOrb = storeDownloadOrb;

async function showStoreDetail(slug) {
  state.storeSearchState.query = $('store-search-query').value;
  state.storeSearchState.tag = storeSelectedTag;
  state.storeSearchState.method = $('store-method-filter').value;
  state.storeView = 'detail';
  
  $('store-browse-view').style.display = 'none';
  
  const detailView = $('store-detail-view');
  detailView.style.display = 'block';
  detailView.innerHTML = `<div class="status-card muted-card">${t('orbsearch.searching')}</div>`;
  
  try {
    const orb = await invoke('store_get_orb', { slug });
    if (!orb) {
      detailView.innerHTML = `<div class="status-card error">${t('store.no_detail')}</div>`;
      return;
    }
    renderStoreDetail(orb);
  } catch (error) {
    detailView.innerHTML = `<div class="status-card error">${escapeHtml(String(error))}</div>`;
  }
}

window.showStoreDetail = showStoreDetail;

function renderStoreDetail(orb) {
  const detailView = $('store-detail-view');
  
  const methodsHtml = (orb.methods || []).map(m => `<span class="store-pill">${escapeHtml(m)}</span>`).join('');
  const tagsHtml = (orb.tags || []).map(t => `<span class="store-pill">${escapeHtml(typeof t === 'string' ? t : t.name || t)}</span>`).join('');
  
  const versionsHtml = (orb.versions || []).map(v => {
    const firstArtifact = (v.artifacts && v.artifacts.length > 0) ? v.artifacts[0] : null;
    const artifactId = firstArtifact ? escapeHtml(firstArtifact.id) : '';
    const hasPwd = v.has_password;
    
    return `
      <div class="store-version-item">
        <div class="version-info">
          <strong>${escapeHtml(v.version)}</strong>
          <span class="muted" style="margin-left:8px;">${formatTimestamp(v.published_at)}</span>
          ${v.is_latest ? `<span class="store-pill" style="margin-left:8px;">Latest</span>` : ''}
          ${hasPwd ? `<span class="store-pill" style="margin-left:8px;">Password</span>` : ''}
        </div>
        <button class="btn btn-primary store-import-btn" data-artifact-id="${artifactId}" data-version="${escapeHtml(v.version)}" data-has-password="${hasPwd}" ${!artifactId ? 'disabled' : ''}>
          Import
        </button>
      </div>
    `;
  }).join('');
  
  detailView.innerHTML = `
    <div class="store-detail-header">
      <button class="btn btn-secondary" onclick="showStoreBrowse()">&larr; ${t('store.detail_back_btn')}</button>
      <h2>${escapeHtml(orb.display_name || orb.slug)}</h2>
      <div class="store-detail-meta">
        <span class="store-pill">v${escapeHtml(orb.latest_version)}</span>
        ${orb.is_private ? `<span class="store-pill">Private</span>` : ''}
      </div>
    </div>
    <p class="store-detail-desc">${escapeHtml(orb.description || '')}</p>
    
    ${methodsHtml ? `<div class="store-detail-section"><strong>Methods:</strong> ${methodsHtml}</div>` : ''}
    ${tagsHtml ? `<div class="store-detail-section"><strong>Tags:</strong> ${tagsHtml}</div>` : ''}
    
    <div class="store-detail-section">
      <h3 style="margin-bottom: 8px;">${t('store.versions_title')}</h3>
      <div class="store-version-list">
        ${versionsHtml || '<div class="muted">No versions</div>'}
      </div>
    </div>
  `;
  
  detailView.querySelectorAll('.store-import-btn').forEach((button) => {
    button.addEventListener('click', () => {
      const artifactId = button.dataset.artifactId;
      const version = button.dataset.version;
      const hasPassword = button.dataset.hasPassword === 'true';
      storeImportOrb(artifactId, hasPassword, version);
    });
  });
}

window.renderStoreDetail = renderStoreDetail;

function showStoreBrowse() {
  state.storeView = 'browse';
  
  $('store-detail-view').style.display = 'none';
  $('store-detail-view').innerHTML = '';
  
  $('store-browse-view').style.display = 'block';
  
  $('store-search-query').value = state.storeSearchState.query || '';
  setStoreTag(state.storeSearchState.tag || null);
  $('store-method-filter').value = state.storeSearchState.method || '';
  
  storeSearch(state.storeSearchState.query, state.storeSearchState.tag, state.storeSearchState.method, state.storeSearchState.page);
}

window.showStoreBrowse = showStoreBrowse;

function storeDownloadArtifact(artifactId, hasPassword) {
  if (hasPassword === true || hasPassword === 'true') {
    $('store-password-dialog').style.display = 'flex';
    $('store-password-input').value = '';
    $('store-password-status').textContent = '';
    $('store-password-status').classList.remove('error');
    $('store-password-input').focus();
    state.pendingDownloadArtifactId = artifactId;
    return;
  }

  setStoreSearchStatus(t('store.downloading', { slug: artifactId }) || 'Downloading...', false);
  invoke('store_download_artifact', { artifactId, token: null })
    .then((result) => {
      setStoreSearchStatus(t('store.artifact_downloaded', { result: result || artifactId }) || `Downloaded: ${result || artifactId}`, false);
    })
    .catch((error) => {
      setStoreSearchStatus(t('store.download_error', { error }), true);
    });
}

window.storeDownloadArtifact = storeDownloadArtifact;

/** Find the import button element for a given artifact ID. */
function getImportBtn(artifactId) {
  return document.querySelector(`.store-import-btn[data-artifact-id="${CSS.escape(artifactId)}"]`);
}

/** Set an import button's text and state. */
function setImportBtnState(btn, textKey, importing) {
  if (!btn) return;
  btn.textContent = t(textKey);
  btn.disabled = !!importing;
  btn.classList.toggle('btn-importing', !!importing);
}

async function storeImportOrb(artifactId, hasDownloadPassword, versionLabel) {
  if (!artifactId) return;
  
  const btn = getImportBtn(artifactId);
  
  if (hasDownloadPassword === true || hasDownloadPassword === 'true') {
    state.pendingDownloadArtifactId = artifactId;
    state.pendingImportMode = true;
    $('store-password-dialog').style.display = 'flex';
    $('store-password-input').value = '';
    $('store-password-status').textContent = '';
    $('store-password-input').focus();
    return;
  }
  
  setImportBtnState(btn, 'store.import_btn_downloading', true);
  setStoreSearchStatus(`Downloading ${versionLabel}...`, false);
  try {
    const path = await invoke('store_download_artifact', { artifactId, token: null });
    setImportBtnState(btn, 'store.import_btn_installing', true);
    setStoreSearchStatus(`Importing ${versionLabel}...`, false);

    const inspect = await invoke('inspect_zip', { path });
    if (inspect.password_protected) {
      state.pendingStoreArtifactId = artifactId;
      showPreImportPasswordDialog(path, inspect.manifest_name);
      return;
    }

    const result = await invoke('import_orb_zip', { path, password: null });
    const name = result.report.manifest.display_name || result.report.manifest.name;
    const ver = result.report.manifest.version;
    setImportBtnState(btn, 'store.import_btn_done', false);
    setStoreSearchStatus(`✅ Imported ${name} v${ver}`, false);
    await refreshLibrary();
    setTimeout(() => showTab('library'), 800);
  } catch (error) {
    setImportBtnState(btn, 'store.import_btn', false);
    setStoreSearchStatus(String(error), true);
  }
}

window.storeImportOrb = storeImportOrb;

function storeSubmitPassword() {
  const pwd = $('store-password-input').value;
  const artifactId = state.pendingDownloadArtifactId;
  if (!pwd || !artifactId) return;

  const btn = getImportBtn(artifactId);
  $('store-password-status').textContent = t('store.password_verifying');
  $('store-password-status').classList.remove('error');

  invoke('store_verify_download_password', { artifactId, password: pwd })
    .then((tokenResult) => {
      $('store-password-dialog').style.display = 'none';
      state.pendingDownloadArtifactId = null;
      const isImport = state.pendingImportMode;
      state.pendingImportMode = false;
      const token = tokenResult && typeof tokenResult === 'object' ? tokenResult.token : tokenResult;
      
      if (isImport) {
        setImportBtnState(btn, 'store.import_btn_downloading', true);
        setStoreSearchStatus('Downloading...', false);
        return invoke('store_download_artifact', { artifactId, token })
          .then(async (path) => {
            setImportBtnState(btn, 'store.import_btn_installing', true);
            setStoreSearchStatus('Importing...', false);

            const inspect = await invoke('inspect_zip', { path });
            if (inspect.password_protected) {
              state.pendingStoreArtifactId = artifactId;
              showPreImportPasswordDialog(path, inspect.manifest_name);
              return;
            }

            const result = await invoke('import_orb_zip', { path, password: null });
            const name = result.report.manifest.display_name || result.report.manifest.name;
            const ver = result.report.manifest.version;
            setImportBtnState(btn, 'store.import_btn_done', false);
            setStoreSearchStatus(`✅ Imported ${name} v${ver}`, false);
            return refreshLibrary().then(() => {
              setTimeout(() => showTab('library'), 800);
            });
          });
      } else {
        setImportBtnState(btn, 'store.import_btn_downloading', true);
        setStoreSearchStatus('Downloading...', false);
        return invoke('store_download_artifact', { artifactId, token })
          .then(() => {
            setImportBtnState(btn, 'store.import_btn', false);
            setStoreSearchStatus('✅ Download complete', false);
          });
      }
    })
    .catch((error) => {
      setImportBtnState(btn, 'store.import_btn', false);
      const errMsg = String(error).toLowerCase();
      let displayMsg;
      if (errMsg.includes('incorrect password') || errMsg.includes('invalid password') || errMsg.includes('wrong password')) {
        displayMsg = t('store.password_incorrect');
      } else if (errMsg.includes('connect') || errMsg.includes('timeout') || errMsg.includes('dns') || errMsg.includes('network')) {
        displayMsg = t('store.password_network_error');
      } else {
        displayMsg = t('store.download_error', { error: String(error) });
      }
      $('store-password-status').textContent = displayMsg;
      $('store-password-status').classList.add('error');
      setStoreSearchStatus(displayMsg, true);
    });
}

function storeCancelPassword() {
  $('store-password-dialog').style.display = 'none';
  state.pendingDownloadArtifactId = null;
}

function storePasswordDialogVisible() {
  return $('store-password-dialog').style.display !== 'none';
}
