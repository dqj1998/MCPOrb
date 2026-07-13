const invoke = window.__TAURI__?.core?.invoke;

const state = {
  orbs: [],
  activeTab: 'library',
  orbSearchTargetId: null,
  platformConfigs: [],
  runningOrbIds: [],
  qaOrbId: null,
  qaPage: 1,
  qaTotalPages: 1,
  pendingDeleteOrbId: null,
};

const importState = {
  selectedPath: null,
};

const $ = (id) => document.getElementById(id);

// ── i18n ──────────────────────────────────────────────────────────────────

const LOCALE_KEY = 'mcporb-runtime-locale';

const locales = {
  en: {
    /* header */
    'app.title': 'MCPOrb Runner',
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
    'library.start_http_btn': 'Start HTTP',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': 'No Orbs installed yet. Click Import to add an Orb ZIP.',
    'library.no_match': 'No Orbs match "{query}".',
    'library.qa_btn': 'Service Requests',
    'library.delete_title': 'Delete Orb',
    'library.delete_btn': 'Delete',
    'library.delete_confirm': 'Are you sure you want to delete "{name}"?',
    'library.delete_success': 'Deleted {name}',
    'library.restart_hint': 'If you have MCP clients (Claude, Cursor, etc.) running, please restart them to use the updated Orbs.',
    'library.stats_requests': 'Req: {total}',
    'library.stats_searches': 'Search: {n}',
    'library.stats_stdio': 'STDIO: {n}',
    'library.stats_http': 'HTTP: {n}',
    /* store */
    'store.title': 'Store',
    'store.search_placeholder': 'Search Orbs in MCP Store',
    'store.search_btn': 'Search',
    'store.no_results': 'No Orbs found.',
    'store.enter_query': 'Enter a search query.',
    'store.download_btn': 'Download',
    'store.downloading': 'Downloading {slug}…',
    'store.downloaded': 'Downloaded and imported {name} {version}',
    'store.results': '{count} result(s)',
    /* running (HTTP gateway) */
    'running.title': 'HTTP',
    'running.refresh': 'Refresh',
    'running.section_desc': 'Gateway HTTP server exposes all installed Orbs as MCP tools through a single endpoint. Copy the config below to connect your MCP client.',
    'running.copy_config_btn': 'Copy Config',
    'running.loading': 'Loading gateway configuration…',
    'running.no_orbs': 'No Orbs installed. The gateway HTTP endpoint is ready — install Orbs from the Library to add tools.',
    /* settings */
    'settings.title': 'Settings',
    'settings.save_btn': 'Save',
    'settings.download_dir_label': 'Download Directory',
    'settings.http_port_label': 'HTTP MCP Port',
    'settings.network_binding_label': 'Network Binding',
    'settings.localhost_opt': 'Localhost (127.0.0.1) — Recommended',
    'settings.external_opt': 'External (0.0.0.0) — Requires caution',
    'settings.saved': 'Settings saved.',
    /* mcp config */
    'mcp.title': 'MCP Config',
    'mcp.runtime_path_label': 'Runtime CLI path',
    'mcp.runtime_path_placeholder': 'Leave blank to use bundled mcporb-runtime',
    'mcp.generate_btn': 'Generate STDIO Config',
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
    'import.success': 'Imported {name} {version}\nStored at {path}\nZIP {zip_sha256}\nAssets {assets_sha256}',
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
  },
  ja: {
    'app.title': 'MCPOrb Runner',
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
    'library.start_http_btn': 'HTTP開始',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': 'インストールされたOrbはありません。「インポート」をクリックしてOrb ZIPを追加してください。',
    'library.no_match': '「{query}」に一致するOrbはありません。',
    'library.qa_btn': 'サービスリクエスト',
    'library.delete_title': 'Orbを削除',
    'library.delete_btn': '削除',
    'library.delete_confirm': '"{name}"を削除してもよろしいですか？',
    'library.delete_success': '{name}を削除しました',
    'library.restart_hint': 'MCPクライアント（Claude、Cursorなど）が起動中の場合は、再起動してOrbの変更を反映してください。',
    'store.title': 'ストア',
    'store.search_placeholder': 'MCP StoreでOrbを検索',
    'store.search_btn': '検索',
    'store.no_results': 'Orbが見つかりません。',
    'store.enter_query': '検索クエリを入力してください。',
    'store.download_btn': 'ダウンロード',
    'store.downloading': '{slug}をダウンロード中…',
    'store.downloaded': '{name} {version}をダウンロードしてインポートしました',
    'store.results': '{count}件の結果',
    'running.title': 'HTTP',
    'running.refresh': '更新',
    'running.section_desc': 'ゲートウェイHTTPサーバーは、インストール済みの全Orbを単一のエンドポイントでMCPツールとして公開します。以下の設定をコピーしてMCPクライアントに接続してください。',
    'running.copy_config_btn': '設定をコピー',
    'running.loading': 'ゲートウェイ設定を読み込み中…',
    'running.no_orbs': 'Orbがインストールされていません。ライブラリからOrbをインストールすると、ゲートウェイHTTPエンドポイントが利用可能になります。',
    /* settings */
    'settings.save_btn': '保存',
    'settings.download_dir_label': 'ダウンロードディレクトリ',
    'settings.http_port_label': 'HTTP MCPポート',
    'settings.network_binding_label': 'ネットワークバインディング',
    'settings.localhost_opt': 'ローカルホスト (127.0.0.1) — 推奨',
    'settings.external_opt': '外部 (0.0.0.0) — 注意が必要',
    'settings.yes_opt': 'はい',
    'settings.no_opt': 'いいえ',
    'settings.saved': '設定を保存しました。',
    'mcp.title': 'MCP設定',
    'mcp.runtime_path_label': 'ランタイムCLIパス',
    'mcp.runtime_path_placeholder': '空白の場合はバンドルされたmcporb-runtimeを使用',
    'mcp.generate_btn': 'STDIO設定を生成',
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
    'import.success': '{name} {version}をインポートしました\n保存先: {path}\nZIP {zip_sha256}\nAssets {assets_sha256}',
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
  },
  zh: {
    'app.title': 'MCPOrb Runner',
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
    'library.start_http_btn': '启动HTTP',
    'library.http_badge': 'HTTP',
    'library.http_btn': 'HTTP',
    'library.no_orbs': '尚未安装Orb。点击"导入"添加Orb ZIP。',
    'library.no_match': '没有匹配"{query}"的Orb。',
    'library.qa_btn': '服务请求',
    'library.delete_title': '删除Orb',
    'library.delete_btn': '删除',
    'library.delete_confirm': '确定要删除"{name}"吗？',
    'library.delete_success': '已删除{name}',
    'library.restart_hint': '如有正在运行中的MCP客户端（Claude、Cursor等），请重启该客户端以使用更新后的Orb。',
    'library.stats_requests': '请求: {total}',
    'library.stats_searches': '搜索: {n}',
    'library.stats_stdio': 'STDIO: {n}',
    'library.stats_http': 'HTTP: {n}',
    'store.title': '商店',
    'store.search_placeholder': '在MCP商店中搜索Orb',
    'store.search_btn': '搜索',
    'store.no_results': '未找到Orb。',
    'store.enter_query': '请输入搜索查询。',
    'store.download_btn': '下载',
    'store.downloading': '正在下载{slug}…',
    'store.downloaded': '已下载并导入{name} {version}',
    'store.results': '{count}个结果',
    'running.title': 'HTTP',
    'running.refresh': '刷新',
    'running.section_desc': '网关HTTP服务器将所有已安装的Orb通过单一端点暴露为MCP工具。复制以下配置连接到您的MCP客户端。',
    'running.copy_config_btn': '复制配置',
    'running.loading': '正在加载网关配置…',
    'running.no_orbs': '尚未安装Orb。从库中安装Orb后，网关HTTP端点即可使用。',
    'settings.title': '设置',
    'settings.save_btn': '保存',
    'settings.download_dir_label': '下载目录',
    'settings.http_port_label': 'HTTP MCP端口',
    'settings.network_binding_label': '网络绑定',
    'settings.localhost_opt': '本地主机 (127.0.0.1) — 推荐',
    'settings.external_opt': '外部 (0.0.0.0) — 需谨慎',
    'settings.yes_opt': '是',
    'settings.no_opt': '否',
    'settings.saved': '设置已保存。',
    'mcp.title': 'MCP配置',
    'mcp.runtime_path_label': '运行时CLI路径',
    'mcp.runtime_path_placeholder': '留空使用内置mcporb-runtime',
    'mcp.generate_btn': '生成STDIO配置',
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
    'import.success': '已导入{name} {version}\n存储位置: {path}\nZIP {zip_sha256}\n资产 {assets_sha256}',
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

window.addEventListener('DOMContentLoaded', async () => {
  initLocale();
  $('lang-select').addEventListener('change', (e) => setLocale(e.target.value));
  bindTabs();
  bindActions();
  setupDragDrop();
  await loadStatus();
  await refreshLibrary();
  await loadSettings();
  await refreshRunning();
  await discoverPlatformConfigs();
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
  $('btn-refresh-running').addEventListener('click', refreshRunning);
  $('btn-discover-configs').addEventListener('click', discoverPlatformConfigs);
  $('btn-store-search').addEventListener('click', storeSearch);
  $('store-search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') storeSearch();
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
  });
  // Orb search modal
  $('btn-orb-search-go').addEventListener('click', runOrbSearch);
  $('orb-search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') runOrbSearch();
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
  $('modal-import-status').textContent = '';
  $('modal-import-status').className = 'status-card muted-card';
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
    // Try Tauri native dialog
    if (window.__TAURI__?.dialog?.open) {
      path = await window.__TAURI__.dialog.open({
        multiple: false,
        filters: [{ name: 'Orb ZIP', extensions: ['zip'] }],
      });
    } else if (window.__TAURI__?.dialog) {
      path = await window.__TAURI__.dialog.open({
        multiple: false,
        filters: [{ name: 'Orb ZIP', extensions: ['zip'] }],
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
  $('modal-import-status').textContent = '';
  $('modal-import-status').className = 'status-card muted-card';
}

function clearSelectedFile() {
  importState.selectedPath = null;
  $('selected-file').style.display = 'none';
  $('drop-zone').style.display = '';
  $('btn-modal-import').disabled = true;
}

async function confirmImport() {
  const path = importState.selectedPath;
  if (!path) return;
  feedbackBtn($('btn-modal-import'), 'feedback.imported');
  setModalStatus(t('import.validating'), false);
  try {
    const result = await invoke('import_orb_zip', { path });
    setModalStatus(
      t('import.success', {
        name: result.report.manifest.display_name || result.report.manifest.name,
        version: result.report.manifest.version,
        path: result.stored_zip_path,
        zip_sha256: result.report.zip_sha256,
        assets_sha256: result.report.assets_sha256,
      }),
      false
    );
    await refreshLibrary();
    showRestartHint();
    // Auto-close after short delay on success
    setTimeout(hideImportModal, 2000);
  } catch (error) {
    setModalStatus(error, true);
    $('btn-modal-import').disabled = false;
  }
}

function setModalStatus(message, isError) {
  const el = $('modal-import-status');
  el.textContent = message;
  el.className = 'status-card' + (isError ? ' error' : '');
}

function showTab(name) {
  state.activeTab = name;
  document.querySelectorAll('.tab-item').forEach((button) => {
    button.classList.toggle('active', button.dataset.tab === name);
  });
  document.querySelectorAll('.tab-panel').forEach((panel) => {
    panel.classList.toggle('active', panel.id === `tab-${name}`);
  });
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
  }
}

async function refreshLibrary() {
  if (!invoke) return renderLibrary([]);
  feedbackBtn($('btn-refresh-library'), 'feedback.refreshed');
  try {
    state.orbs = await invoke('list_orbs');
    renderLibrary(state.orbs);
    syncOrbSelects();
  } catch (error) {
    $('library-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
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
    return;
  }
  $('library-list').innerHTML = filtered.map((orb) => {
    return `
    <article class="orb-card">
      <div>
        <div class="orb-title">${escapeHtml(orb.display_name)}</div>
        <div class="orb-meta">${escapeHtml(orb.install_source)} · ${orb.encrypted_assets ? 'encrypted' : 'plaintext'}</div>
        <div class="orb-desc">${escapeHtml(orb.description || 'No description')}</div>
        <div class="orb-hash">zip ${escapeHtml(orb.zip_sha256)}<br>assets ${escapeHtml(orb.assets_sha256)}</div>
        <div class="orb-stats-row" id="stats-${escapeHtml(orb.id)}"><span class="muted">—</span></div>
      </div>
      <div style="display:flex;gap:8px;">
        <button class="btn btn-secondary" data-search-orb="${escapeHtml(orb.id)}">${t('library.search_btn')}</button>
        <button class="btn btn-secondary" data-qa-orb="${escapeHtml(orb.id)}">${t('library.qa_btn')}</button>
        <button class="btn btn-danger" data-delete-orb="${escapeHtml(orb.id)}">${t('library.delete_btn')}</button>
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
  filtered.forEach((orb) => {
    fetchAndRenderStats(orb.id);
  });
  togglePlatformConfigsSection();
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
  renderLibrary(state.orbs);
}

// ── Orb search modal ──────────────────────────────────────────────────────

function orbSearchModalVisible() {
  return $('orb-search-modal').style.display !== 'none';
}

function showOrbSearchModal(orbId) {
  state.orbSearchTargetId = orbId || null;
  // Set title to include the orb display name
  const orb = state.orbs.find((o) => o.id === orbId);
  $('orb-search-title').textContent = orb
    ? `${t('orbsearch.title')} — ${orb.display_name}`
    : t('orbsearch.title');
  $('orb-search-query').value = '';
  $('orb-search-results').innerHTML = '';
  $('orb-search-results').scrollTop = 0;
  $('orb-search-status').textContent = '';
  $('orb-search-status').className = 'status-line';
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
  feedbackBtn($('btn-orb-search-go'), 'feedback.filtered');
  setOrbSearchStatus(t('orbsearch.searching'), false);
  $('orb-search-results').innerHTML = '';
  try {
    const response = await invoke('search_orb', {
      orbId,
      query,
      method: $('orb-search-method').value,
      topK: 50,
    });
    setOrbSearchStatus(`${response.hits.length} hit(s) · ${response.active_plan}`, false);
    renderOrbSearchResults(response.hits);
  } catch (error) {
    setOrbSearchStatus(error, true);
  }
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
    <article class="result-item">
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
      <article class="qa-entry">
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
  // Accept both ISO 8601 UTC strings and epoch-seconds strings
  if (!isoStr) return '—';
  if (isoStr.includes('T')) {
    // Parse ISO UTC string and render in the user's local timezone
    const d = new Date(isoStr);
    if (!isNaN(d.getTime())) {
      return d.toLocaleString(undefined, {
        year: 'numeric', month: '2-digit', day: '2-digit',
        hour: '2-digit', minute: '2-digit',
      });
    }
    // Fallback: strip fractional seconds and replace T with space
    return isoStr.replace(/\.\d+Z$/, 'Z').replace(/T/, ' ');
  }
  // Try as epoch seconds
  const secs = parseInt(isoStr, 10);
  if (!isNaN(secs) && secs > 1000000000) {
    const d = new Date(secs * 1000);
    return d.toLocaleString();
  }
  return isoStr;
}

async function generateMcpConfig() {
  feedbackBtn($('btn-generate-config'), 'feedback.generated');
  try {
    const snippets = await invoke('gateway_mcp_config_snippets');
    $('mcp-config-list').innerHTML = snippets.map((snippet) => `
      <article class="config-card">
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
    $('settings-download-dir').value = settings.download_dir || '';
    $('settings-http-port').value = settings.http_port || 5599;
    $('settings-network-binding').value = settings.network_binding || 'localhost';
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
}

async function saveSettings() {
  if (!invoke) return;
  feedbackBtn($('btn-save-settings'), 'feedback.saved');
  const settings = {
    download_dir: $('settings-download-dir').value,
    http_port: parseInt($('settings-http-port').value, 10) || 5599,
    network_binding: $('settings-network-binding').value,
  };
  try {
    await invoke('save_settings', { settings });
    $('settings-status').textContent = t('settings.saved');
    $('settings-status').classList.remove('error');
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
    <article class="config-card">
      <div class="config-card-header">
        <div class="config-meta">MCPOrb Gateway HTTP</div>
        <button class="btn btn-secondary btn-sm" id="copy-gateway-http-config">${t('running.copy_config_btn')}</button>
      </div>
      <textarea readonly id="gateway-http-config-json">${t('running.loading')}</textarea>
    </article>
  `;
  // Fetch and populate the gateway HTTP config
  (async () => {
    try {
      const snippets = await invoke('gateway_http_config_snippets');
      const area = $('gateway-http-config-json');
      if (snippets.length > 0) {
        area.value = snippets[0].json;
      } else {
        area.value = '/* Gateway config unavailable */';
      }
      const copyBtn = $('copy-gateway-http-config');
      copyBtn.addEventListener('click', async () => {
        if (snippets.length > 0) {
          await navigator.clipboard.writeText(snippets[0].json);
          feedbackBtn(copyBtn, 'feedback.copied');
        }
      });
    } catch (error) {
      $('gateway-http-config-json').value = `/* Error: ${error} */`;
    }
  })();
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
      <article class="platform-config-card">
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
          <button class="btn btn-primary" data-apply-config="${escapeHtml(cfg.config_path)}" data-platform="${escapeHtml(cfg.platform)}" data-restart-hint-key="${escapeHtml(cfg.restart_hint || '')}" ${!cfg.exists || isSame || !generatedContent ? 'disabled' : ''}>${t('mcp.apply_btn')}</button>
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

async function startOrbHttp(orbId) {
  if (!invoke) return;
  try {
    const result = await invoke('start_orb_http', { orbId });
    await refreshRunning();
    showTab('running');
  } catch (error) {
    alert(`Failed to start Orb: ${error}`);
  }
}

window.startOrbHttp = startOrbHttp;

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
  $('confirm-modal').style.display = 'none';
}

function confirmModalVisible() {
  return $('confirm-modal').style.display !== 'none';
}

async function confirmDeleteOrb() {
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

async function storeSearch() {
  const query = $('store-search-query').value.trim();
  if (!query) {
    setStoreSearchStatus(t('store.enter_query'), true);
    return;
  }
  feedbackBtn($('btn-store-search'), 'feedback.filtered');
  setStoreSearchStatus(`${t('orbsearch.searching')}...`, false);
  $('store-search-results').innerHTML = '';
  try {
    const result = await invoke('store_search', { query });
    setStoreSearchStatus(t('store.results', { count: result.orbs.length }), false);
    renderStoreResults(result.orbs);
  } catch (error) {
    setStoreSearchStatus(String(error), true);
  }
}

function setStoreSearchStatus(message, isError) {
  $('store-search-status').textContent = message;
  $('store-search-status').classList.toggle('error', Boolean(isError));
}

function renderStoreResults(orbs) {
  if (!orbs.length) {
    $('store-search-results').innerHTML = `<div class="status-card muted-card">${t('store.no_results')}</div>`;
    return;
  }
  $('store-search-results').innerHTML = orbs.map((orb) => `
    <article class="orb-card">
      <div>
        <div class="orb-title">${escapeHtml(orb.display_name || orb.name)}</div>
        <div class="orb-meta">${escapeHtml(orb.version)} · ${escapeHtml(orb.tags.join(', '))} · ${orb.has_password ? 'password-protected' : 'public'}</div>
        <div class="orb-desc">${escapeHtml(orb.description || 'No description')}</div>
        <div class="orb-hash">sha256 ${escapeHtml(orb.sha256)}</div>
      </div>
      <button class="btn btn-primary" onclick="feedbackBtn(this,'feedback.started');storeDownloadOrb('${escapeHtml(orb.slug)}', ${orb.has_password})">${t('store.download_btn')}</button>
    </article>
  `).join('');
}

async function storeDownloadOrb(slug, hasPassword) {
  let password = null;
  if (hasPassword) {
    password = prompt(`Enter download password for "${slug}":`);
    if (password === null) return;
  }
  setStoreSearchStatus(t('store.downloading', { slug }), false);
  try {
    const result = await invoke('store_download_orb', { slug, password });
    const name = result.report.manifest.display_name || result.report.manifest.name;
    const version = result.report.manifest.version;
    setStoreSearchStatus(t('store.downloaded', { name, version }), false);
    await refreshLibrary();
    showRestartHint();
    showTab('library');
  } catch (error) {
    setStoreSearchStatus(String(error), true);
  }
}

window.storeDownloadOrb = storeDownloadOrb;
