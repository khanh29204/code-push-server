// ------------------------ Data loading ------------------------

async function loadApps() {
  const generation = ++state.appLoadGeneration;
  const data = await api('GET', '/apps');
  if (generation !== state.appLoadGeneration) return;
  state.apps = data.apps || [];
  renderAppList();
}

async function loadDeployments(appName) {
  const generation = ++state.deploymentLoadGeneration;
  const data = await api('GET', `/apps/${encodeURIComponent(appName)}/deployments`);
  if (generation !== state.deploymentLoadGeneration || state.currentApp?.name !== appName) return;
  state.deployments = data.deployments || [];
  renderDeploymentTabs();
}

async function loadHistory() {
  const app = state.currentApp, dep = state.currentDeployment;
  if (!app || !dep) return;
  const appName = app.name, deploymentName = dep.name;
  const generation = ++state.historyLoadGeneration;
  const limit = document.getElementById('historyLimit').value || 100;
  const base = `/apps/${encodeURIComponent(appName)}/deployments/${encodeURIComponent(deploymentName)}`;
  const [hist, met, detail] = await Promise.all([
    api('GET', `${base}/history?limit=${limit}`),
    api('GET', `${base}/metrics`),
    api('GET', base),
  ]);
  if (generation !== state.historyLoadGeneration || state.currentApp?.name !== appName || state.currentDeployment?.name !== deploymentName) return;
  if (!detail?.deployment || detail.deployment.name !== deploymentName) throw new Error('Invalid deployment details returned by the server');
  if (!Array.isArray(hist?.history) || !met?.metrics || typeof met.metrics !== 'object' || Array.isArray(met.metrics)) throw new Error('Invalid release history or metrics returned by the server');
  state.history = hist.history;
  state.metrics = met.metrics;
  state.deployments = state.deployments.map(item => item.name === deploymentName ? detail.deployment : item);
  state.currentDeployment = detail.deployment;
  pruneSelectedReleases();
  renderHistory();
}
