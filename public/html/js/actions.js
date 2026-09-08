// ------------------------ Selection ------------------------

async function selectApp(name) {
  const app = state.apps.find(a => a.name === name);
  if (!app) return;
  state.currentApp = app;
  state.currentDeployment = null;
  state.history = [];
  state.metrics = {};
  renderAppList();
  renderDeploymentTabs();
  try {
    await loadDeployments(name);
    if (state.deployments.length) {
      const preferred = state.deployments.find(d => d.name === 'Production') || state.deployments[0];
      await selectDeployment(preferred.name);
    }
  } catch (e) { toast(e.message, 'error'); }
}

async function selectDeployment(name) {
  const dep = state.deployments.find(d => d.name === name);
  if (!dep) return;
  state.currentDeployment = dep;
  renderDeploymentTabs();
  try { await loadHistory(); } catch (e) { toast(e.message, 'error'); }
}

// ------------------------ Actions: app / deployment ------------------------

async function createApp() {
  const v = await prompt2(t('createAppTitle'), [
    { name: 'name', label: t('appName'), placeholder: 'MyApp' },
    { name: 'os', label: t('os'), type: 'select', options: [
      { value: 'android', label: 'android' }, { value: 'ios', label: 'ios' }, { value: 'windows', label: 'windows' },
    ], value: 'android' },
    { name: 'platform', label: t('platform'), type: 'select', options: [
      { value: 'react-native', label: 'react-native' }, { value: 'cordova', label: 'cordova' },
    ], value: 'react-native' },
  ], t('create'));
  if (!v || !v.name) return;
  try {
    await api('POST', '/apps', v);
    toast(t('appCreated'), 'success');
    await loadApps();
    await selectApp(v.name);
  } catch (e) { toast(e.message, 'error'); }
}

async function renameApp() {
  if (!state.currentApp) return;
  const v = await prompt2(t('renameApp'), [{ name: 'name', label: t('newName'), value: state.currentApp.name }], t('rename'));
  if (!v || !v.name || v.name === state.currentApp.name) return;
  try {
    await api('PATCH', `/apps/${encodeURIComponent(state.currentApp.name)}`, { name: v.name });
    toast(t('renamed'), 'success');
    state.currentApp = { ...state.currentApp, name: v.name };
    await loadApps();
    await selectApp(v.name);
  } catch (e) { toast(e.message, 'error'); }
}

async function deleteApp() {
  if (!state.currentApp) return;
  const ok = await confirm2(t('deleteApp'), t('deleteAppConfirm', state.currentApp.name));
  if (!ok) return;
  try {
    await api('DELETE', `/apps/${encodeURIComponent(state.currentApp.name)}`);
    toast(t('appDeleted'), 'success');
    state.currentApp = null;
    state.currentDeployment = null;
    state.deployments = [];
    await loadApps();
    renderDeploymentTabs();
    renderHistory();
  } catch (e) { toast(e.message, 'error'); }
}

async function createDeployment() {
  if (!state.currentApp) return;
  const v = await prompt2(t('createDeploymentTitle'), [{ name: 'name', label: t('deploymentName'), placeholder: 'Staging' }], t('create'));
  if (!v || !v.name) return;
  try {
    await api('POST', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments`, { name: v.name });
    toast(t('deploymentCreated'), 'success');
    await loadDeployments(state.currentApp.name);
    await selectDeployment(v.name);
  } catch (e) { toast(e.message, 'error'); }
}

async function renameDeployment() {
  if (!state.currentDeployment) return;
  const v = await prompt2(t('renameDeployment'), [{ name: 'name', label: t('newName'), value: state.currentDeployment.name }], t('rename'));
  if (!v || !v.name || v.name === state.currentDeployment.name) return;
  try {
    await api('PATCH', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}`, { name: v.name });
    toast(t('renamed'), 'success');
    await loadDeployments(state.currentApp.name);
    await selectDeployment(v.name);
  } catch (e) { toast(e.message, 'error'); }
}

async function deleteDeployment() {
  if (!state.currentDeployment) return;
  const ok = await confirm2(t('deleteDeployment'), t('deleteDeploymentConfirm', state.currentDeployment.name));
  if (!ok) return;
  try {
    await api('DELETE', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}`);
    toast(t('deleted'), 'success');
    state.currentDeployment = null;
    await loadDeployments(state.currentApp.name);
    if (state.deployments.length) await selectDeployment(state.deployments[0].name);
    else renderHistory();
  } catch (e) { toast(e.message, 'error'); }
}

async function clearHistory() {
  if (!state.currentDeployment) return;
  const ok = await confirm2(t('clearHistory'), t('clearHistoryConfirm', state.currentDeployment.name));
  if (!ok) return;
  try {
    await api('DELETE', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/history`);
    toast(t('historyCleared'), 'success');
    await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
}

async function rollbackLatest() {
  if (!state.currentDeployment) return;
  const ok = await confirm2(t('rollbackLatest'), t('rollbackLatestConfirm', state.currentDeployment.name));
  if (!ok) return;
  try {
    await api('POST', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/rollback`);
    toast(t('rolledBack'), 'success');
    await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
}

async function promotePackage() {
  if (!state.currentDeployment) return;
  const others = state.deployments.filter(d => d.name !== state.currentDeployment.name);
  if (!others.length) return toast(t('noOtherDep'), 'error');
  const v = await prompt2(t('promoteTitle', state.currentDeployment.name), [
    { name: 'dest', label: t('destDeployment'), type: 'select', options: others.map(d => ({ value: d.name, label: d.name })) },
    { name: 'description', label: t('descOverride'), type: 'textarea' },
    { name: 'rollout', label: t('rolloutPct'), type: 'number', min: 1, max: 100, value: 100 },
    { name: 'isMandatory', type: 'checkbox', checkboxLabel: t('mandatory') },
  ], t('promote'));
  if (!v) return;
  const body = { packageInfo: { rollout: v.rollout ?? 100, isMandatory: !!v.isMandatory } };
  if (v.description) body.packageInfo.description = v.description;
  try {
    await api('POST', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/promote/${encodeURIComponent(v.dest)}`, body);
    toast(t('promoted'), 'success');
    await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
}

async function handleReleaseAction(act, label) {
  const p = state.history.find(x => x.label === label);
  if (!p) return;
  if (act === 'edit') {
    const v = await prompt2(t('editRelease', label), [
      { name: 'description', label: t('description'), type: 'textarea', value: p.description || '' },
      { name: 'rollout', label: t('rolloutPct'), type: 'number', min: 1, max: 100, value: p.rollout ?? 100 },
      { name: 'isMandatory', type: 'checkbox', checkboxLabel: t('mandatory'), value: !!p.isMandatory },
      { name: 'isDisabled', type: 'checkbox', checkboxLabel: t('disabled'), value: !!p.isDisabled },
    ], t('save'));
    if (!v) return;
    const body = {
      packageInfo: {
        label: label,
        description: v.description,
        rollout: v.rollout,
        isMandatory: !!v.isMandatory,
        isDisabled: !!v.isDisabled,
      }
    };
    try {
      await api('PATCH', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/release`, body);
      toast(t('updated'), 'success');
      await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
  } else if (act === 'rollback') {
    const ok = await confirm2(t('rollback'), t('rollbackConfirm', state.currentDeployment.name, label));
    if (!ok) return;
    try {
      await api('POST', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/rollback/${encodeURIComponent(label)}`);
      toast(t('rolledBack'), 'success');
      await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
  } else if (act === 'delete-files') {
    const ok = await confirm2(t('delFilesTitle'), t('delFilesConfirm', label));
    if (!ok) return;
    try {
      const r = await api('DELETE', `/storage/audit?label=${encodeURIComponent(label)}`);
      toast(t('delFilesDone', r.deletedCount, r.matchedPackages, r.freedSize), 'success');
      await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
  } else if (act === 'delete') {
    const ok = await confirm2(t('deleteRelease'), t('deleteReleaseConfirm', label));
    if (!ok) return;
    try {
      await api('DELETE', `/apps/${encodeURIComponent(state.currentApp.name)}/deployments/${encodeURIComponent(state.currentDeployment.name)}/releases/${encodeURIComponent(label)}`);
      toast(t('deleted'), 'success');
      await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
  }
}
