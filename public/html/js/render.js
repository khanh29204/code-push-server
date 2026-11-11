// ------------------------ Rendering ------------------------

function renderAppList() {
  const list = document.getElementById('appList');
  if (!state.apps.length) {
    list.innerHTML = `<div class="text-center text-xs text-onSurface-variant py-6">${t('noApps')}<br><button id="createFirstApp" class="mt-2 text-primary hover:underline">${t('createOne')}</button></div>`;
    document.getElementById('createFirstApp')?.addEventListener('click', createApp);
    return;
  }
  list.innerHTML = state.apps.map(a => {
    const active = state.currentApp && state.currentApp.name === a.name;
    return `<button data-app="${escapeHtml(a.name)}" class="w-full flex items-center justify-between text-left rounded-full px-4 py-2.5 text-sm transition ${active ? 'bg-secondary-container text-secondary-onContainer font-medium' : 'text-onSurface-variant hover:bg-surface-containerHigh'}">
      <span class="truncate">${escapeHtml(a.name)}</span>
      ${a.os ? `<span class="ml-2 text-[10px] uppercase tracking-wide text-outline">${escapeHtml(a.os)}</span>` : ''}
    </button>`;
  }).join('');
  list.querySelectorAll('[data-app]').forEach(b => b.onclick = () => { if (!state.busy) selectApp(b.dataset.app); });
}

function renderDeploymentTabs() {
  const tabs = document.getElementById('deploymentTabs');
  document.getElementById('appActions').classList.toggle('hidden', !state.currentApp);
  if (!state.currentApp) { tabs.innerHTML = ''; return; }
  syncBusyControls();
  document.getElementById('currentAppTitle').textContent = state.currentApp.name;
  document.getElementById('currentAppMeta').textContent = [state.currentApp.os, state.currentApp.platform].filter(Boolean).join(' · ');
  if (!state.deployments.length) {
    tabs.innerHTML = `<button id="newDepBtnEmpty" ${state.busy ? 'disabled' : ''} class="rounded-full border border-dashed border-outline px-4 py-1.5 text-xs text-onSurface-variant hover:border-primary hover:text-primary">${t('createDeployment')}</button>`;
    document.getElementById('newDepBtnEmpty').onclick = createDeployment;
    document.getElementById('toolbar').classList.add('hidden');
    document.getElementById('emptyState').classList.remove('hidden');
    document.getElementById('tableWrap').classList.add('hidden');
    return;
  }
  tabs.innerHTML = state.deployments.map(d => {
    const active = state.currentDeployment && state.currentDeployment.name === d.name;
    return `<button data-dep="${escapeHtml(d.name)}" class="px-4 py-2.5 text-sm border-b-2 -mb-px transition ${active ? 'border-primary text-primary font-medium' : 'border-transparent text-onSurface-variant hover:text-onSurface'}">${escapeHtml(d.name)}</button>`;
  }).join('') + `<button id="newDepBtnInline" ${state.busy ? 'disabled' : ''} class="ml-2 m3-icon-btn h-9 w-9" title="${t('createDeployment')}"><span class="material-symbols-rounded" style="font-size:20px">add</span></button>`;
  tabs.querySelectorAll('[data-dep]').forEach(b => { b.disabled = state.busy; b.onclick = () => { if (!state.busy) selectDeployment(b.dataset.dep); }; });
  document.getElementById('newDepBtnInline').onclick = createDeployment;
}

function syncBusyControls() {
  for (const id of ['refreshBtn', 'historyLimit', 'promoteBtn', 'rollbackLatestBtn', 'clearHistoryBtn', 'renameDepBtn', 'deleteDepBtn', 'renameAppBtn', 'deleteAppBtn', 'newAppBtn', 'appSettingsBtn', 'accessKeysBtn', 'storageAuditBtn', 'diagnosticsBtn']) {
    const control = document.getElementById(id);
    if (control) control.disabled = state.busy;
  }
  document.querySelectorAll('#appList [data-app]').forEach(control => { control.disabled = state.busy; });
  document.querySelectorAll('#deploymentTabs button').forEach(control => { control.disabled = state.busy; });
}

function flagBadge(label, on, color) {
  if (!on) return '';
  const map = {
    red: 'bg-error-container text-error-onContainer border-error-container',
    amber: 'bg-tertiary-container text-tertiary-onContainer border-tertiary-container',
    slate: 'bg-secondary-container text-secondary-onContainer border-secondary-container',
  };
  return `<span class="inline-flex items-center rounded-full border px-2 py-0.5 text-[10px] font-medium ${map[color]}">${escapeHtml(label)}</span>`;
}

function renderHistory() {
  syncBusyControls();
  const wrap = document.getElementById('tableWrap');
  const empty = document.getElementById('emptyState');
  const toolbar = document.getElementById('toolbar');
  if (!state.currentDeployment) { wrap.classList.add('hidden'); empty.classList.remove('hidden'); toolbar.classList.add('hidden'); renderBulkBar(); return; }
  toolbar.classList.remove('hidden');
  document.getElementById('depKey').textContent = state.currentDeployment.key || '';
  if (!state.history.length) {
    wrap.classList.add('hidden');
    empty.classList.remove('hidden');
    empty.querySelector('p').textContent = t('emptyReleases');
    renderBulkBar();
    return;
  }
  empty.classList.add('hidden');
  wrap.classList.remove('hidden');
  const rows = displayedReleaseHistory().map(p => {
    const m = state.metrics[p.label] || {};
    const checked = state.selectedLabels.has(p.label);
    const disabled = state.busy ? 'disabled' : '';
    const flags = [
      flagBadge(t('flagMandatory'), p.isMandatory, 'red'),
      flagBadge(t('flagDisabled'), p.isDisabled, 'slate'),
      (p.rollout != null && p.rollout < 100) ? flagBadge(t('flagRollout', p.rollout), true, 'amber') : '',
      p.releaseMethod && p.releaseMethod !== 'Upload' ? flagBadge(p.releaseMethod.toUpperCase(), true, 'slate') : '',
    ].filter(Boolean).join(' ');
    const metrics = `<span class="text-[#1E4620] font-medium">${m.active ?? 0}</span> / <span>${m.installed ?? 0}</span> / <span class="text-onSurface-variant">${m.downloaded ?? 0}</span> / <span class="text-error">${m.failed ?? 0}</span>`;
    return `<tr class="hover:bg-surface-containerHigh/60">
      <td class="px-3 py-2.5"><input type="checkbox" data-select-label="${escapeHtml(p.label)}" aria-label="${escapeHtml(t('selectRelease', p.label))}" ${checked ? 'checked' : ''} ${disabled} class="accent-primary"></td>
      <td class="px-3 py-2.5 font-mono text-xs font-semibold text-onSurface"><button data-act="detail" data-label="${escapeHtml(p.label)}" ${disabled} class="hover:underline" aria-label="${escapeHtml(t('releaseDetails', p.label))}">${escapeHtml(p.label)}</button></td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant">${escapeHtml(p.appVersion || '')}</td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant max-w-xs truncate" title="${escapeHtml(p.description || '')}">${escapeHtml(p.description || '')}</td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant" title="${new Date(p.uploadTime || 0).toString()}"><div>${fmtTime(p.uploadTime)}</div><div class="text-[10px] text-outline">${escapeHtml(p.releasedBy || '')}</div></td>
      <td class="px-3 py-2.5 text-xs text-right text-onSurface-variant">${p.rollout != null ? p.rollout + '%' : '—'}</td>
      <td class="px-3 py-2.5 text-center space-x-1">${flags || '<span class="text-outline-variant">—</span>'}</td>
      <td class="px-3 py-2.5 text-xs text-right text-onSurface-variant">${fmtBytes(p.size)}</td>
      <td class="px-3 py-2.5 text-xs text-right font-mono">${metrics}</td>
      <td class="px-3 py-2.5 text-right whitespace-nowrap">
        <button data-act="edit" data-label="${escapeHtml(p.label)}" ${disabled} class="text-xs text-primary hover:underline">${t('edit')}</button>
        <button data-act="rollback" data-label="${escapeHtml(p.label)}" ${disabled} class="ml-2 text-xs text-tertiary hover:underline">${t('rollback')}</button>
        <button data-act="cleanup" data-label="${escapeHtml(p.label)}" ${disabled} class="ml-2 text-xs text-onSurface-variant hover:text-error hover:underline">${t('cleanSelected')}</button>
      </td>
    </tr>`;
  }).join('');
  document.getElementById('historyBody').innerHTML = rows;
  const result = state.cleanupResults && state.cleanupResults.generation === state.contextGeneration && state.cleanupResults.appName === state.currentApp.name && state.cleanupResults.deploymentName === state.currentDeployment.name ? state.cleanupResults : null;
  const resultsRoot = document.getElementById('cleanupResultsContainer');
  resultsRoot.classList.toggle('hidden', !result);
  resultsRoot.replaceChildren();
  if (result) {
    const panel = document.createElement('section');
    panel.className = 'm-3 max-h-56 overflow-y-auto rounded-lg border border-outline-variant bg-surface-containerLow p-3 text-xs';
    panel.setAttribute('aria-live', 'polite');
    panel.innerHTML = `<h3 class="mb-2 font-medium">Cleanup results · ${escapeHtml(fmtBytes(result.freedBytes))}</h3>${result.results.map(item => `<p>${escapeHtml(item.label)}: ${escapeHtml(item.status)}${item.errors?.length ? ` (${escapeHtml(item.errors.join('; '))})` : ''}</p>`).join('')}`;
    resultsRoot.appendChild(panel);
  }
  document.querySelectorAll('#historyBody [data-select-label]').forEach(input => {
    input.onchange = () => { toggleSelectedRelease(input.dataset.selectLabel, input.checked); renderBulkBar(); };
  });
  document.querySelectorAll('#historyBody [data-act]').forEach(button => {
    button.onclick = () => handleReleaseAction(button.dataset.act, button.dataset.label);
  });
  renderBulkBar();
}

function renderBulkBar() {
  const root = document.getElementById('bulkActions');
  if (!root) return;
  if (!state.currentDeployment) { root.innerHTML = ''; return; }
  const labels = selectedReleaseLabels();
  const allLabels = [...displayedReleaseLabels()];
  const allSelected = allLabels.length > 0 && allLabels.every(label => state.selectedLabels.has(label));
  const someSelected = allLabels.some(label => state.selectedLabels.has(label));
  root.innerHTML = `<div class="flex items-center gap-2 flex-wrap"><label class="inline-flex items-center gap-2 text-xs text-onSurface-variant"><input id="selectAllReleases" type="checkbox" aria-label="${escapeHtml(t('selectAllReleases'))}" ${allSelected ? 'checked' : ''} ${state.busy ? 'disabled' : ''} class="accent-primary"><span>${escapeHtml(t('selectAll'))}</span></label><span class="text-xs text-onSurface-variant">${escapeHtml(t('selectedCount', labels.length))}</span><button id="bulkEditBtn" ${!labels.length || state.busy ? 'disabled' : ''} class="m3-btn m3-btn-tonal text-xs py-1 px-3">${escapeHtml(t('editSelected'))}</button><button id="bulkCleanupBtn" ${!labels.length || state.busy ? 'disabled' : ''} class="m3-btn m3-btn-outlined text-xs py-1 px-3" style="color:#B3261E;border-color:#B3261E">${escapeHtml(t('cleanSelected'))}</button><button id="clearSelectionBtn" ${!labels.length || state.busy ? 'disabled' : ''} class="m3-btn m3-btn-text text-xs py-1 px-3">${escapeHtml(t('clearSelection'))}</button></div>`;
  const selectAll = document.getElementById('selectAllReleases');
  if (selectAll) {
    selectAll.indeterminate = someSelected && !allSelected;
    selectAll.onchange = () => { selectDisplayedReleases(selectAll.checked); renderHistory(); };
  }
  document.getElementById('bulkEditBtn')?.addEventListener('click', bulkEditSelectedReleases);
  document.getElementById('bulkCleanupBtn')?.addEventListener('click', () => cleanSelectedReleaseFiles());
  document.getElementById('clearSelectionBtn')?.addEventListener('click', () => { clearSelectedReleases(); renderHistory(); });
}
