// ------------------------ Selection ------------------------

async function selectApp(name) {
  const app = state.apps.find(a => a.name === name);
  if (!app || state.busy) return;
  const appLoad = ++state.appLoadGeneration;
  ++state.contextGeneration;
  ++state.historyLoadGeneration;
  state.currentApp = app;
  state.currentDeployment = null;
  state.deployments = [];
  state.history = [];
  state.metrics = {};
  clearSelectedReleases();
  renderAppList();
  renderDeploymentTabs();
  renderHistory();
  try {
    await loadDeployments(name);
    if (state.appLoadGeneration !== appLoad || state.currentApp?.name !== name) return;
    if (state.deployments.length) {
      const preferred = state.deployments.find(d => d.name === 'Production') || state.deployments[0];
      await selectDeployment(preferred.name);
    }
  } catch (e) { toast(e.message, 'error'); }
}

async function selectDeployment(name) {
  const dep = state.deployments.find(d => d.name === name);
  if (!dep || state.busy) return;
  ++state.contextGeneration;
  ++state.historyLoadGeneration;
  state.currentDeployment = dep;
  state.cleanupResults = null;
  state.history = [];
  state.metrics = {};
  clearSelectedReleases();
  renderDeploymentTabs();
  renderHistory();
  try { await loadHistory(); } catch (e) { toast(e.message, 'error'); }
}

// ------------------------ Actions: app / deployment ------------------------

async function createApp() {
  if (state.busy) return;
  const v = await prompt2(t('createAppTitle'), [
    { name: 'name', label: t('appName'), placeholder: 'MyApp' },
    { name: 'os', label: t('os'), type: 'select', options: [
      { value: 'Android', label: 'Android' }, { value: 'iOS', label: 'iOS' }, { value: 'Windows', label: 'Windows' },
    ], value: 'Android' },
    { name: 'platform', label: t('platform'), type: 'select', options: [
      { value: 'React-Native', label: 'React-Native' }, { value: 'Cordova', label: 'Cordova' },
    ], value: 'React-Native' },
  ], t('create'));
  if (!v || !v.name) return;
  const context = state.contextGeneration;
  state.busy = true;
  renderHistory();
  try {
    await api('POST', '/apps', v);
    toast(t('appCreated'), 'success');
    await loadApps();
    if (state.contextGeneration === context) {
      state.busy = false;
      await selectApp(v.name);
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function renameApp() {
  const app = state.currentApp;
  if (!app || state.busy) return;
  const targetAppName = app.name;
  const context = state.contextGeneration;
  const v = await prompt2(t('renameApp'), [{ name: 'name', label: t('newName'), value: app.name }], t('rename'));
  if (!v || !v.name || v.name === app.name || state.currentApp?.name !== targetAppName || state.contextGeneration !== context) return;
  state.busy = true;
  renderHistory();
  try {
    await api('PATCH', `/apps/${encodeURIComponent(targetAppName)}`, { name: v.name });
    toast(t('renamed'), 'success');
    await loadApps();
    if (state.contextGeneration === context && state.currentApp?.name === targetAppName) {
      state.currentApp = { ...state.currentApp, name: v.name };
      ++state.contextGeneration;
      state.busy = false;
      await selectApp(v.name);
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function deleteApp() {
  const appName = state.currentApp?.name;
  if (!appName || state.busy) return;
  const context = state.contextGeneration;
  const ok = await confirm2(t('deleteApp'), t('deleteAppConfirm', appName));
  if (!ok || state.currentApp?.name !== appName || state.contextGeneration !== context) return;
  state.busy = true;
  renderHistory();
  try {
    await api('DELETE', `/apps/${encodeURIComponent(appName)}`);
    toast(t('appDeleted'), 'success');
    await loadApps();
    if (state.contextGeneration === context && state.currentApp?.name === appName) {
      ++state.contextGeneration;
      state.currentApp = null;
      state.currentDeployment = null;
      state.deployments = [];
      renderDeploymentTabs();
      renderHistory();
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function createDeployment() {
  if (!state.currentApp || state.busy) return;
  const targetAppName = state.currentApp.name;
  const context = state.contextGeneration;
  const v = await prompt2(t('createDeploymentTitle'), [{ name: 'name', label: t('deploymentName'), placeholder: 'Staging' }], t('create'));
  if (!v || !v.name || state.currentApp?.name !== targetAppName || state.contextGeneration !== context) return;
  state.busy = true;
  renderHistory();
  try {
    await api('POST', `/apps/${encodeURIComponent(targetAppName)}/deployments`, { name: v.name });
    toast(t('deploymentCreated'), 'success');
    if (state.currentApp?.name === targetAppName && state.contextGeneration === context) {
      await loadDeployments(targetAppName);
      state.busy = false;
      await selectDeployment(v.name);
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function renameDeployment() {
  const target = captureReleaseTarget();
  if (!target || state.busy) return;
  const v = await prompt2(t('renameDeployment'), [{ name: 'name', label: t('newName'), value: target.deploymentName }], t('rename'));
  if (!v || !v.name || v.name === target.deploymentName || !releaseTargetIsCurrent(target)) return;
  state.busy = true;
  renderHistory();
  try {
    await api('PATCH', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}`, { name: v.name });
    toast(t('renamed'), 'success');
    if (releaseTargetIsCurrent(target)) {
      await loadDeployments(target.appName);
      state.currentDeployment = null;
      state.busy = false;
      await selectDeployment(v.name);
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function deleteDeployment() {
  const target = captureReleaseTarget();
  if (!target || state.busy) return;
  const ok = await confirm2(t('deleteDeployment'), t('deleteDeploymentConfirm', target.deploymentName));
  if (!ok || !releaseTargetIsCurrent(target)) return;
  state.busy = true;
  renderHistory();
  try {
    await api('DELETE', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}`);
    toast(t('deleted'), 'success');
    if (releaseTargetIsCurrent(target)) {
      state.currentDeployment = null;
      await loadDeployments(target.appName);
      if (state.deployments.length) {
        state.busy = false;
        await selectDeployment(state.deployments[0].name);
      } else renderHistory();
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function clearHistory() {
  const target = captureReleaseTarget();
  if (!target || state.busy) return;
  const ok = await confirm2(t('clearHistory'), t('clearHistoryConfirm', target.deploymentName));
  if (!ok || !releaseTargetIsCurrent(target)) return;
  state.busy = true;
  renderHistory();
  try {
    await api('DELETE', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/history`);
    toast(t('historyCleared'), 'success');
    if (releaseTargetIsCurrent(target)) await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function rollbackLatest() {
  const target = captureReleaseTarget();
  if (!target || state.busy) return;
  const ok = await confirm2(t('rollbackLatest'), t('rollbackLatestConfirm', target.deploymentName));
  if (!ok || !releaseTargetIsCurrent(target)) return;
  state.busy = true;
  renderHistory();
  try {
    await api('POST', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/rollback`);
    toast(t('rolledBack'), 'success');
    if (releaseTargetIsCurrent(target)) await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function promotePackage() {
  const target = captureReleaseTarget();
  if (!target || state.busy) return;
  const source = state.currentDeployment;
  const others = state.deployments.filter(d => d.name !== source.name);
  if (!others.length) return toast(t('noOtherDep'), 'error');
  const v = await prompt2(t('promoteTitle', source.name), [
    { name: 'sourceLabel', label: t('sourceLabel'), type: 'text', placeholder: t('latestRelease'), value: '' },
    { name: 'dest', label: t('destDeployment'), type: 'select', options: others.map(d => ({ value: d.name, label: d.name })) },
    { name: 'description', label: t('descOverride'), type: 'textarea' },
    { name: 'appVersion', label: t('appVersionOverride'), placeholder: t('appVersionWarning') },
    { name: 'rollout', label: t('rolloutPct'), type: 'number', min: 1, max: 100, value: '' },
    { name: 'isMandatory', label: t('mandatory'), type: 'select', value: 'keep', options: [
      { value: 'keep', label: t('keep') }, { value: 'on', label: t('setOn') }, { value: 'off', label: t('setOff') },
    ] },
    { name: 'isDisabled', label: t('disabled'), type: 'select', value: 'keep', options: [
      { value: 'keep', label: t('keep') }, { value: 'on', label: t('setOn') }, { value: 'off', label: t('setOff') },
    ] },
  ], t('promote'));
  if (!v || !releaseTargetIsCurrent(target)) return;
  const packageInfo = {};
  if (v.sourceLabel) packageInfo.label = v.sourceLabel;
  if (v.description) packageInfo.description = v.description;
  if (v.appVersion) packageInfo.appVersion = v.appVersion;
  if (v.rollout != null && (!Number.isInteger(v.rollout) || v.rollout < 1 || v.rollout > 100)) return toast(t('invalidRollout'), 'error');
  if (v.rollout != null) packageInfo.rollout = v.rollout;
  if (v.isMandatory !== 'keep' && v.isMandatory != null) packageInfo.isMandatory = v.isMandatory === 'on';
  if (v.isDisabled !== 'keep' && v.isDisabled != null) packageInfo.isDisabled = v.isDisabled === 'on';
  const historyLoad = ++state.historyLoadGeneration;
  state.busy = true;
  renderHistory();
  try {
    await api('POST', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/promote/${encodeURIComponent(v.dest)}`, { packageInfo });
    toast(t('promoted'), 'success');
    if (releaseTargetIsCurrent(target)) await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function bulkEditSelectedReleases() {
  const target = captureReleaseTarget();
  const labels = selectedReleaseLabels();
  if (!target || !labels.length || state.busy) return;
  const body = await prompt2(t('bulkEditTitle'), [
    { name: 'isDisabled', label: t('disabled'), type: 'select', value: 'keep', options: [
      { value: 'keep', label: t('keep') }, { value: 'on', label: t('setOn') }, { value: 'off', label: t('setOff') },
    ] },
    { name: 'isMandatory', label: t('mandatory'), type: 'select', value: 'keep', options: [
      { value: 'keep', label: t('keep') }, { value: 'on', label: t('setOn') }, { value: 'off', label: t('setOff') },
    ] },
    { name: 'rollout', label: t('rolloutPct'), type: 'number', min: 1, max: 100, step: 1, value: '', hint: t('rolloutKeep') },
  ], t('editSelected'));
  if (body === null || !releaseTargetIsCurrent(target)) return;
  if (body.rollout != null && (!Number.isInteger(body.rollout) || body.rollout < 1 || body.rollout > 100)) return toast(t('invalidRollout'), 'error');
  const packageInfo = buildBulkPackageInfo(body);
  const changed = Object.entries(packageInfo).map(([key, value]) => `${key}: ${value}`).join(', ');
  if (!changed) return toast(t('noChanges'), 'error');
  const warning = packageInfo.rollout == null ? '' : ` ${t('rolloutWarning')}`;
  const ok = await confirm2(t('bulkEditTitle'), `${t('bulkEditConfirm', target.appName, target.deploymentName, labels.join(', '), changed)}${warning}`, false);
  if (!ok || !releaseTargetIsCurrent(target) || state.busy) return;
  const historyLoad = ++state.historyLoadGeneration;
  state.busy = true;
  renderHistory();
  try {
    await api('PATCH', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/releases/bulk`, { labels, packageInfo });
    toast(t('updated'), 'success');
    if (releaseTargetIsCurrent(target)) await loadHistory();
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function cleanReleaseFiles(labels, target = captureReleaseTarget()) {
  if (!target || !labels.length || state.busy) return;
  const contextKey = `${target.appName}\0${target.deploymentName}`;
  const selectedDeployment = state.deployments.find(d => d.name === target.deploymentName);
  const currentLabel = selectedDeployment?.package?.label;
  if (currentLabel && labels.includes(currentLabel)) {
    const current = state.history.find(p => p.label === currentLabel);
    if (!current?.isDisabled) {
      toast(t('disableCurrentFirst', currentLabel), 'error');
      return;
    }
  }
  const ok = await confirm2(t('cleanSelected'), t('cleanConfirm', target.appName, target.deploymentName, labels.join(', ')));
  if (!ok || !releaseTargetIsCurrent(target) || state.busy) return;
  const historyLoad = ++state.historyLoadGeneration;
  state.busy = true;
  renderHistory();
  try {
    const result = await api('DELETE', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/releases/bulk`, { labels });
    const counts = { cleaned: 0, skipped: 0, missing: 0, error: 0 };
    if (releaseTargetIsCurrent(target)) state.cleanupResults = { ...target, results: result.results || [], freedBytes: result.freedBytes || 0 };
    for (const item of result.results || []) {
      counts[item.status] = (counts[item.status] || 0) + 1;
    }
    toast(`${t('cleanupResults', counts.cleaned, counts.skipped, counts.missing, counts.error)} ${t('freedBytes', fmtBytes(result.freedBytes || 0))}`, counts.error || counts.skipped ? 'error' : 'success');
    if (releaseTargetIsCurrent(target) && state.selectionTarget === contextKey) {
      for (const item of result.results || []) if (item.status === 'cleaned' || item.status === 'missing') state.selectedLabels.delete(item.label);
    }
    if (state.historyLoadGeneration === historyLoad && releaseTargetIsCurrent(target)) {
      await loadHistory();
    }
  } catch (e) { toast(e.message, 'error'); }
  finally { state.busy = false; renderHistory(); }
}

async function cleanSelectedReleaseFiles() {
  return cleanReleaseFiles(selectedReleaseLabels());
}
async function openReleaseDetail(label) {
  const target = captureReleaseTarget();
  const release = state.history.find(item => item.label === label);
  if (!target || !release) return;
  try {
    const pkg = await api('GET', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/releases/${encodeURIComponent(label)}`);
    if (state.busy || !releaseTargetIsCurrent(target)) return;
    const links = [];
    const addLink = (name, value) => {
      if (typeof value !== 'string' || !value) return;
      let url;
      try {
        url = new URL(value, location.origin);
      } catch { return; }
      if (!['http:', 'https:'].includes(url.protocol)) return;
      links.push(`<a class="text-primary underline" href="${escapeHtml(url.href)}" target="_blank" rel="noopener noreferrer">${escapeHtml(name)}</a>`);
    };
    addLink(t('packageBlob'), release.blobUrl);
    addLink(t('manifestBlob'), release.manifestBlobUrl);
    for (const [baseHash, diff] of Object.entries(release.diffPackageMap || {})) {
      addLink(`${t('diffBlob')} · ${baseHash}`, diff?.url);
    }
    const details = [
      `${t('thLabel')}: ${pkg.label || release.label}`,
      `${t('thAppVersion')}: ${release.appVersion || '—'}`,
      `${t('thDescription')}: ${release.description || '—'}`,
      `${t('thReleased')}: ${fmtTime(release.uploadTime)}`,
      `${t('thRollout')}: ${release.rollout ?? '—'}`,
      `${t('mandatory')}: ${!!release.isMandatory}`,
      `${t('disabled')}: ${!!release.isDisabled}`,
      `${t('thSize')}: ${fmtBytes(release.size)}`,
      `${t('thMetrics')}: active ${state.metrics[label]?.active ?? 0}, installed ${state.metrics[label]?.installed ?? 0}, downloaded ${state.metrics[label]?.downloaded ?? 0}, failed ${state.metrics[label]?.failed ?? 0}`,
      `${t('packageHash')}: ${pkg.packageHash || pkg.package_hash || '—'}`,
      ...((pkg.originalLabel || pkg.original_label) ? [`${t('originalLabel')}: ${pkg.originalLabel || pkg.original_label}`] : []),
      ...((pkg.originalDeployment || pkg.original_deployment) ? [`${t('originalDeployment')}: ${pkg.originalDeployment || pkg.original_deployment}`] : []),
      ...((pkg.releasedBy || release.releasedBy) ? [`${t('releasedBy')}: ${pkg.releasedBy || release.releasedBy}`] : []),
    ];
    modal({
      title: t('releaseDetails', label),
      body: `<div class="space-y-2 text-sm">${details.map(line => `<p>${escapeHtml(line)}</p>`).join('')}${links.length ? `<p class="flex gap-3">${links.join('')}</p>` : ''}</div>`,
      actions: [{ label: t('close'), kind: 'secondary', value: null }],
    });
  } catch (e) { toast(e.message, 'error'); }
}

async function handleReleaseAction(act, label) {
  if (state.busy) return;
  const target = captureReleaseTarget();
  const p = state.history.find(x => x.label === label);
  if (!p || !target) return;
  if (act === 'detail') return openReleaseDetail(label);
  if (act === 'cleanup') return cleanReleaseFiles([label], target);
  if (act === 'edit') {
    const v = await prompt2(t('editRelease', label), [
      { name: 'description', label: t('description'), type: 'textarea', value: p.description || '' },
      { name: 'appVersion', label: t('appVersionOverride'), value: p.appVersion || '', hint: t('appVersionWarning') },
      { name: 'rollout', label: t('rolloutPct'), type: 'number', min: 1, max: 100, value: p.rollout ?? 100 },
      { name: 'isMandatory', type: 'checkbox', checkboxLabel: t('mandatory'), value: !!p.isMandatory },
      { name: 'isDisabled', type: 'checkbox', checkboxLabel: t('disabled'), value: !!p.isDisabled },
    ], t('save'));
    if (!v || !releaseTargetIsCurrent(target)) return;
    if (v.rollout == null || !Number.isInteger(v.rollout) || v.rollout < 1 || v.rollout > 100) return toast(t('invalidRollout'), 'error');
    const packageInfo = { label, description: v.description, rollout: v.rollout, isMandatory: !!v.isMandatory, isDisabled: !!v.isDisabled };
    const originalAppVersion = p.appVersion || '';
    if (v.appVersion !== originalAppVersion) packageInfo.appVersion = v.appVersion;
    const enabling = !!p.isDisabled && v.isDisabled === false;
    if (enabling && (
      v.description !== (p.description || '') || v.rollout !== (p.rollout ?? 100) ||
      !!v.isMandatory !== !!p.isMandatory || v.appVersion !== originalAppVersion
    )) return toast(t('enableOnly'), 'error');
    const historyLoad = ++state.historyLoadGeneration;
    state.busy = true;
    renderHistory();
    try {
      const endpoint = enabling
        ? `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/releases/bulk`
        : `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/release`;
      const payload = enabling
        ? { labels: [label], packageInfo: { isDisabled: false } }
        : { packageInfo };
      await api('PATCH', endpoint, payload);
      toast(t('updated'), 'success');
      if (releaseTargetIsCurrent(target)) await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
    finally { state.busy = false; renderHistory(); }
  } else if (act === 'rollback') {
    const ok = await confirm2(t('rollback'), t('rollbackConfirm', target.deploymentName, label));
    if (!ok || !releaseTargetIsCurrent(target)) return;
    const historyLoad = ++state.historyLoadGeneration;
    state.busy = true;
    renderHistory();
    try {
      await api('POST', `/apps/${encodeURIComponent(target.appName)}/deployments/${encodeURIComponent(target.deploymentName)}/rollback/${encodeURIComponent(label)}`);
      toast(t('rolledBack'), 'success');
      if (releaseTargetIsCurrent(target)) await loadHistory();
    } catch (e) { toast(e.message, 'error'); }
    finally { state.busy = false; renderHistory(); }
  }
}
