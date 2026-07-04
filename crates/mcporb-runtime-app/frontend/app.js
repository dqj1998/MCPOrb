const invoke = window.__TAURI__?.core?.invoke;

const state = {
  orbs: [],
  activeTab: 'search',
};

const $ = (id) => document.getElementById(id);

window.addEventListener('DOMContentLoaded', async () => {
  bindTabs();
  bindActions();
  await loadStatus();
  await refreshLibrary();
  await loadSettings();
  await refreshRunning();
  bindDeepLinkListeners();
});

function bindTabs() {
  document.querySelectorAll('.tab-item').forEach((button) => {
    button.addEventListener('click', () => showTab(button.dataset.tab));
  });
}

function bindActions() {
  $('btn-refresh-library').addEventListener('click', refreshLibrary);
  $('btn-import').addEventListener('click', importOrbZip);
  $('btn-search').addEventListener('click', runSearch);
  $('search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') runSearch();
  });
  $('btn-generate-config').addEventListener('click', generateMcpConfig);
  $('btn-save-settings').addEventListener('click', saveSettings);
  $('btn-refresh-running').addEventListener('click', refreshRunning);
  $('btn-store-search').addEventListener('click', storeSearch);
  $('store-search-query').addEventListener('keydown', (event) => {
    if (event.key === 'Enter') storeSearch();
  });
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
    $('app-version').textContent = 'static preview';
    $('registry-path').textContent = 'Tauri runtime unavailable';
    return;
  }
  try {
    const status = await invoke('runtime_status');
    $('app-version').textContent = `v${status.version}`;
    $('registry-path').textContent = status.registry_dir;
    $('settings-status').innerHTML = `
      <div class="status-card"><strong>Store</strong><br>${escapeHtml(status.store_status)}</div>
      <div class="status-card"><strong>HTTP MCP</strong><br>${escapeHtml(status.http_mcp_status)}</div>
      <div class="status-card"><strong>Registry</strong><br>${escapeHtml(status.registry_dir)}</div>
    `;
  } catch (error) {
    $('registry-path').textContent = String(error);
  }
}

async function refreshLibrary() {
  if (!invoke) return renderLibrary([]);
  try {
    state.orbs = await invoke('list_orbs');
    renderLibrary(state.orbs);
    syncOrbSelects();
  } catch (error) {
    $('library-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
}

function renderLibrary(orbs) {
  if (!orbs.length) {
    $('library-list').innerHTML = '<div class="status-card muted-card">No Orbs installed yet. Import an Orb ZIP to start.</div>';
    return;
  }
  $('library-list').innerHTML = orbs.map((orb) => `
    <article class="orb-card">
      <div>
        <div class="orb-title">${escapeHtml(orb.display_name)}</div>
        <div class="orb-meta">${escapeHtml(orb.version)} · ${escapeHtml(orb.install_source)} · ${orb.encrypted_assets ? 'encrypted' : 'plaintext'}</div>
        <div class="orb-desc">${escapeHtml(orb.description || 'No description')}</div>
        <div class="orb-hash">zip ${escapeHtml(orb.zip_sha256)}<br>assets ${escapeHtml(orb.assets_sha256)}</div>
      </div>
      <div style="display:flex;gap:8px;">
        <button class="btn btn-secondary" data-search-orb="${escapeHtml(orb.id)}">Search</button>
        <button class="btn btn-primary" data-start-orb="${escapeHtml(orb.id)}">Start HTTP</button>
      </div>
    </article>
  `).join('');
  document.querySelectorAll('[data-search-orb]').forEach((button) => {
    button.addEventListener('click', () => {
      $('search-orb-select').value = button.dataset.searchOrb;
      showTab('search');
    });
  });
  document.querySelectorAll('[data-start-orb]').forEach((button) => {
    button.addEventListener('click', () => startOrbHttp(button.dataset.startOrb));
  });
}

function syncOrbSelects() {
  const options = state.orbs.map((orb) => `<option value="${escapeHtml(orb.id)}">${escapeHtml(orb.display_name)} ${escapeHtml(orb.version)}</option>`).join('');
  for (const select of [$('search-orb-select'), $('mcp-orb-select')]) {
    const previous = select.value;
    select.innerHTML = options || '<option value="">No installed Orbs</option>';
    if (previous) select.value = previous;
  }
}

async function importOrbZip() {
  const path = $('import-path').value.trim();
  if (!path) {
    setImportStatus('Paste an Orb ZIP path first.', true);
    return;
  }
  setImportStatus('Validating and importing Orb ZIP...', false);
  try {
    const result = await invoke('import_orb_zip', { path });
    setImportStatus(`Imported ${result.report.manifest.display_name || result.report.manifest.name} ${result.report.manifest.version}\nStored at ${result.stored_zip_path}\nZIP ${result.report.zip_sha256}\nAssets ${result.report.assets_sha256}`, false);
    await refreshLibrary();
    showTab('library');
  } catch (error) {
    setImportStatus(error, true);
  }
}

function setImportStatus(message, isError) {
  $('import-status').textContent = message;
  $('import-status').classList.toggle('error', Boolean(isError));
}

async function runSearch() {
  const orbId = $('search-orb-select').value;
  const query = $('search-query').value.trim();
  if (!orbId || !query) {
    setSearchStatus('Choose an Orb and enter a query.', true);
    return;
  }
  setSearchStatus('Searching...', false);
  $('search-results').innerHTML = '';
  try {
    const response = await invoke('search_orb', {
      orbId,
      query,
      method: $('search-method').value,
      topK: 8,
    });
    setSearchStatus(`${response.hits.length} hit(s) · ${response.active_plan}`, false);
    renderSearchResults(response.hits);
  } catch (error) {
    setSearchStatus(error, true);
  }
}

function setSearchStatus(message, isError) {
  $('search-status').textContent = message;
  $('search-status').classList.toggle('error', Boolean(isError));
}

function renderSearchResults(hits) {
  if (!hits.length) {
    $('search-results').innerHTML = '<div class="status-card muted-card">No matches.</div>';
    return;
  }
  $('search-results').innerHTML = hits.map((hit) => `
    <article class="result-item">
      <div class="result-meta">${escapeHtml(hit.document_title)}${hit.page ? ` · p.${hit.page}` : ''} · ${escapeHtml(hit.method)} · ${Number(hit.score).toFixed(3)}</div>
      <div class="result-text">${escapeHtml(hit.text)}</div>
    </article>
  `).join('');
}

async function generateMcpConfig() {
  const orbId = $('mcp-orb-select').value;
  if (!orbId) return;
  const runtimeBinary = $('runtime-bin-path').value.trim() || null;
  try {
    const snippets = await invoke('mcp_config_snippets', { orbId, runtimeBinary });
    $('mcp-config-list').innerHTML = snippets.map((snippet) => `
      <article class="config-card">
        <div class="config-meta">${escapeHtml(snippet.label)}</div>
        <textarea readonly>${escapeHtml(snippet.json)}</textarea>
      </article>
    `).join('');
  } catch (error) {
    $('mcp-config-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
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
    $('settings-auto-start').value = settings.auto_start ? 'true' : 'false';
  } catch (error) {
    console.error('Failed to load settings:', error);
  }
}

async function saveSettings() {
  if (!invoke) return;
  const settings = {
    download_dir: $('settings-download-dir').value,
    http_port: parseInt($('settings-http-port').value, 10) || 5599,
    network_binding: $('settings-network-binding').value,
    auto_start: $('settings-auto-start').value === 'true',
  };
  try {
    await invoke('save_settings', { settings });
    $('settings-status').textContent = 'Settings saved.';
    $('settings-status').classList.remove('error');
  } catch (error) {
    $('settings-status').textContent = String(error);
    $('settings-status').classList.add('error');
  }
}

async function refreshRunning() {
  if (!invoke) return renderRunning([]);
  try {
    const running = await invoke('list_running_orbs');
    renderRunning(running);
  } catch (error) {
    $('running-list').innerHTML = `<div class="status-card error">${escapeHtml(error)}</div>`;
  }
}

function renderRunning(running) {
  if (!running.length) {
    $('running-list').innerHTML = '<div class="status-card muted-card">No Orbs running. Use the Library to start an Orb HTTP server.</div>';
    return;
  }
  $('running-list').innerHTML = running.map((r) => `
    <article class="orb-card">
      <div>
        <div class="orb-title">${escapeHtml(r.orb_id)}</div>
        <div class="orb-meta">Port ${r.port} · PID ${r.pid}</div>
        <div class="orb-hash">http://127.0.0.1:${r.port}/${escapeHtml(r.token)}/</div>
      </div>
      <div style="display:flex;gap:8px;">
        <button class="btn btn-secondary" onclick="copyHttpConfig('${escapeHtml(r.orb_id)}')">Copy Config</button>
        <button class="btn btn-primary" onclick="stopOrbHttp('${escapeHtml(r.orb_id)}')">Stop</button>
      </div>
    </article>
  `).join('');
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

async function copyHttpConfig(orbId) {
  if (!invoke) return;
  try {
    const snippets = await invoke('mcp_config_http_snippets', { orbId });
    if (snippets.length > 0) {
      await navigator.clipboard.writeText(snippets[0].json);
      alert('HTTP MCP config copied to clipboard.');
    }
  } catch (error) {
    alert(`Failed to copy config: ${error}`);
  }
}

window.copyHttpConfig = copyHttpConfig;

function bindDeepLinkListeners() {
  if (!window.__TAURI__?.event?.listen) return;

  window.__TAURI__.event.listen('runtime:deep-link-import', async (event) => {
    const zipPath = event.payload;
    if (!zipPath) return;
    setImportStatus(`Importing from deep link: ${zipPath}...`, false);
    showTab('import');
    $('import-path').value = zipPath;
    try {
      const result = await invoke('import_orb_zip', { path: zipPath });
      setImportStatus(`Imported ${result.report.manifest.display_name || result.report.manifest.name} ${result.report.manifest.version}`, false);
      await refreshLibrary();
      showTab('library');
    } catch (error) {
      setImportStatus(error, true);
    }
  });

  window.__TAURI__.event.listen('runtime:deep-link-install', async (event) => {
    const { slug, version } = event.payload || {};
    showTab('store');
    if (slug) {
      $('store-search-query').value = slug;
      await storeSearch();
    }
  });
}

async function storeSearch() {
  const query = $('store-search-query').value.trim();
  if (!query) {
    setStoreSearchStatus('Enter a search query.', true);
    return;
  }
  setStoreSearchStatus('Searching Store...', false);
  $('store-search-results').innerHTML = '';
  try {
    const result = await invoke('store_search', { query });
    setStoreSearchStatus(`${result.orbs.length} result(s)`, false);
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
    $('store-search-results').innerHTML = '<div class="status-card muted-card">No Orbs found.</div>';
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
      <button class="btn btn-primary" onclick="storeDownloadOrb('${escapeHtml(orb.slug)}', ${orb.has_password})">Download</button>
    </article>
  `).join('');
}

async function storeDownloadOrb(slug, hasPassword) {
  let password = null;
  if (hasPassword) {
    password = prompt(`Enter download password for "${slug}":`);
    if (password === null) return;
  }
  setStoreSearchStatus(`Downloading ${slug}...`, false);
  try {
    const result = await invoke('store_download_orb', { slug, password });
    setStoreSearchStatus(`Downloaded and imported ${result.report.manifest.display_name || result.report.manifest.name} ${result.report.manifest.version}`, false);
    await refreshLibrary();
    showTab('library');
  } catch (error) {
    setStoreSearchStatus(String(error), true);
  }
}

window.storeDownloadOrb = storeDownloadOrb;
