// ------------------------ Utilities ------------------------

// Shared app state — mutated by data.js / actions.js, read by render.js
const state = {
  apps: [],
  currentApp: null,
  deployments: [],
  currentDeployment: null,
  history: [],
  metrics: {},
  selectedLabels: new Set(),
  selectionTarget: null,
  contextGeneration: 0,
  appLoadGeneration: 0,
  deploymentLoadGeneration: 0,
  historyLoadGeneration: 0,
  busy: false,
};

function releaseContextKey(app = state.currentApp, dep = state.currentDeployment) {
  return app && dep ? `${app.name}\0${dep.name}` : null;
}

function clearSelectedReleases() {
  state.selectedLabels.clear();
  state.selectionTarget = releaseContextKey();
}

function syncReleaseSelectionContext() {
  const key = releaseContextKey();
  if (state.selectionTarget !== key) {
    state.selectedLabels.clear();
    state.selectionTarget = key;
  }
}

function selectDisplayedReleases(checked) {
  syncReleaseSelectionContext();
  const labels = displayedReleaseLabels();
  if (checked) for (const label of labels) state.selectedLabels.add(label);
  else for (const label of labels) state.selectedLabels.delete(label);
}

function displayedReleaseLabels() {
  return new Set(displayedReleaseHistory().map(p => p.label));
}

function displayedReleaseHistory() {
  const limit = Number(document.getElementById('historyLimit').value) || 100;
  const seen = new Set();
  return state.history.slice().sort((a, b) => (b.uploadTime || 0) - (a.uploadTime || 0)).filter(p => {
    if (!p.label || seen.has(p.label)) return false;
    seen.add(p.label);
    return true;
  }).slice(0, limit);
}

function toggleSelectedRelease(label, checked) {
  syncReleaseSelectionContext();
  if (checked) state.selectedLabels.add(label);
  else state.selectedLabels.delete(label);
}

function pruneSelectedReleases() {
  syncReleaseSelectionContext();
  const labels = new Set(state.history.map(p => p.label).filter(Boolean));
  state.selectedLabels = new Set([...state.selectedLabels].filter(label => labels.has(label)));
}

function selectedReleaseLabels() {
  syncReleaseSelectionContext();
  const labels = displayedReleaseLabels();
  return [...state.selectedLabels].filter(label => labels.has(label));
}

function captureReleaseTarget() {
  return state.currentApp && state.currentDeployment
    ? { appName: state.currentApp.name, deploymentName: state.currentDeployment.name, generation: state.contextGeneration }
    : null;
}

function releaseTargetIsCurrent(target) {
  return !!target && state.contextGeneration === target.generation && state.currentApp?.name === target.appName && state.currentDeployment?.name === target.deploymentName;
}

function buildBulkPackageInfo(form) {
  const packageInfo = {};
  for (const field of ['isDisabled', 'isMandatory']) {
    if (form[field] === 'on') packageInfo[field] = true;
    else if (form[field] === 'off') packageInfo[field] = false;
  }
  if (form.rollout !== '' && form.rollout != null) {
    const rollout = Number(form.rollout);
    if (Number.isInteger(rollout) && rollout >= 1 && rollout <= 100) packageInfo.rollout = rollout;
  }
  return packageInfo;
}

const token = () => localStorage.getItem('auth');
if (!token()) location.href = '/auth/login';

function toast(msg, kind = 'info') {
  const el = document.createElement('div');
  const colors = {
    info: 'bg-surface-inverse text-onSurface-inverse',
    success: 'bg-[#1E4620] text-white',
    error: 'bg-error text-error-on',
  };
  el.className = `rounded-lg shadow-m3-3 px-4 py-3 text-sm ${colors[kind] || colors.info} animate-in`;
  el.textContent = msg;
  document.getElementById('toastRoot').appendChild(el);
  setTimeout(() => el.remove(), 3500);
}

async function api(method, path, body, opts = {}) {
  const headers = { 'Authorization': 'Bearer ' + token() };
  let payload;
  if (body !== undefined && body !== null) {
    if (opts.form) {
      const fd = new URLSearchParams();
      for (const k in body) fd.append(k, body[k]);
      payload = fd.toString();
      headers['Content-Type'] = 'application/x-www-form-urlencoded';
    } else {
      headers['Content-Type'] = 'application/json';
      payload = JSON.stringify(body);
    }
  }
  const res = await fetch(path, { method, headers, body: payload });
  if (res.status === 401) {
    localStorage.removeItem('auth');
    location.href = '/auth/login';
    throw new Error('Unauthorized');
  }
  const text = await res.text();
  let data = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = text; }
  const status = data && typeof data.status === 'string' ? data.status.toUpperCase() : '';
  if (!res.ok || status === 'ERROR') {
    const msg = (data && (data.message || data.error)) || text || `HTTP ${res.status}`;
    throw new Error(msg);
  }
  return data;
}

function pad2(n) { return n < 10 ? '0' + n : '' + n; }

function fmtTime(ms) {
  if (!ms) return '—';
  const d = new Date(ms);
  return `${pad2(d.getDate())}/${pad2(d.getMonth() + 1)}/${d.getFullYear()} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

function fmtBytes(n) {
  if (!n && n !== 0) return '—';
  if (n < 1024) return n + ' B';
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + ' KB';
  if (n < 1024 * 1024 * 1024) return (n / 1024 / 1024).toFixed(2) + ' MB';
  return (n / 1024 / 1024 / 1024).toFixed(2) + ' GB';
}

function escapeHtml(s) {
  if (s == null) return '';
  return String(s).replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}
