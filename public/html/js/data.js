// ------------------------ Data loading ------------------------

async function loadApps() {
  const data = await api('GET', '/apps');
  state.apps = data.apps || [];
  renderAppList();
}

async function loadDeployments(appName) {
  const data = await api('GET', `/apps/${encodeURIComponent(appName)}/deployments`);
  state.deployments = data.deployments || [];
  renderDeploymentTabs();
}

async function loadHistory() {
  const app = state.currentApp, dep = state.currentDeployment;
  if (!app || !dep) return;
  const limit = document.getElementById('historyLimit').value || 100;
  const [hist, met] = await Promise.all([
    api('GET', `/apps/${encodeURIComponent(app.name)}/deployments/${encodeURIComponent(dep.name)}/history?limit=${limit}`),
    api('GET', `/apps/${encodeURIComponent(app.name)}/deployments/${encodeURIComponent(dep.name)}/metrics`).catch(() => ({ metrics: {} })),
  ]);
  state.history = hist.history || [];
  state.metrics = met.metrics || {};
  renderHistory();
}
