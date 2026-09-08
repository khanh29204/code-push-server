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
  list.querySelectorAll('[data-app]').forEach(b => b.onclick = () => selectApp(b.dataset.app));
}

function renderDeploymentTabs() {
  const tabs = document.getElementById('deploymentTabs');
  document.getElementById('appActions').classList.toggle('hidden', !state.currentApp);
  if (!state.currentApp) { tabs.innerHTML = ''; return; }
  document.getElementById('currentAppTitle').textContent = state.currentApp.name;
  document.getElementById('currentAppMeta').textContent = [state.currentApp.os, state.currentApp.platform].filter(Boolean).join(' · ');
  if (!state.deployments.length) {
    tabs.innerHTML = `<button id="newDepBtnEmpty" class="rounded-full border border-dashed border-outline px-4 py-1.5 text-xs text-onSurface-variant hover:border-primary hover:text-primary">${t('createDeployment')}</button>`;
    document.getElementById('newDepBtnEmpty').onclick = createDeployment;
    document.getElementById('toolbar').classList.add('hidden');
    document.getElementById('emptyState').classList.remove('hidden');
    document.getElementById('tableWrap').classList.add('hidden');
    return;
  }
  tabs.innerHTML = state.deployments.map(d => {
    const active = state.currentDeployment && state.currentDeployment.name === d.name;
    return `<button data-dep="${escapeHtml(d.name)}" class="px-4 py-2.5 text-sm border-b-2 -mb-px transition ${active ? 'border-primary text-primary font-medium' : 'border-transparent text-onSurface-variant hover:text-onSurface'}">${escapeHtml(d.name)}</button>`;
  }).join('') + `<button id="newDepBtnInline" class="ml-2 m3-icon-btn h-9 w-9" title="${t('createDeployment')}"><span class="material-symbols-rounded" style="font-size:20px">add</span></button>`;
  tabs.querySelectorAll('[data-dep]').forEach(b => b.onclick = () => selectDeployment(b.dataset.dep));
  document.getElementById('newDepBtnInline').onclick = createDeployment;
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
  const wrap = document.getElementById('tableWrap');
  const empty = document.getElementById('emptyState');
  const toolbar = document.getElementById('toolbar');
  if (!state.currentDeployment) { wrap.classList.add('hidden'); empty.classList.remove('hidden'); toolbar.classList.add('hidden'); return; }
  toolbar.classList.remove('hidden');
  document.getElementById('depKey').textContent = state.currentDeployment.key || '';
  if (!state.history.length) {
    wrap.classList.add('hidden');
    empty.classList.remove('hidden');
    empty.querySelector('p').textContent = t('emptyReleases');
    return;
  }
  empty.classList.add('hidden');
  wrap.classList.remove('hidden');
  const rows = state.history.slice().sort((a, b) => (b.uploadTime || 0) - (a.uploadTime || 0)).map(p => {
    const m = state.metrics[p.label] || {};
    const flags = [
      flagBadge(t('flagMandatory'), p.isMandatory, 'red'),
      flagBadge(t('flagDisabled'), p.isDisabled, 'slate'),
      (p.rollout != null && p.rollout < 100) ? flagBadge(t('flagRollout', p.rollout), true, 'amber') : '',
      p.releaseMethod && p.releaseMethod !== 'Upload' ? flagBadge(p.releaseMethod.toUpperCase(), true, 'slate') : '',
    ].filter(Boolean).join(' ');
    const metrics = `<span class="text-[#1E4620] font-medium">${m.active ?? 0}</span> / <span>${m.installed ?? 0}</span> / <span class="text-onSurface-variant">${m.downloaded ?? 0}</span> / <span class="text-error">${m.failed ?? 0}</span>`;
    return `<tr class="hover:bg-surface-containerHigh/60">
      <td class="px-3 py-2.5 font-mono text-xs font-semibold text-onSurface">${escapeHtml(p.label)}</td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant">${escapeHtml(p.appVersion || '')}</td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant max-w-xs truncate" title="${escapeHtml(p.description || '')}">${escapeHtml(p.description || '')}</td>
      <td class="px-3 py-2.5 text-xs text-onSurface-variant" title="${new Date(p.uploadTime || 0).toString()}">
        <div>${fmtTime(p.uploadTime)}</div>
        <div class="text-[10px] text-outline">${escapeHtml(p.releasedBy || '')}</div>
      </td>
      <td class="px-3 py-2.5 text-xs text-right text-onSurface-variant">${p.rollout != null ? p.rollout + '%' : '—'}</td>
      <td class="px-3 py-2.5 text-center space-x-1">${flags || '<span class="text-outline-variant">—</span>'}</td>
      <td class="px-3 py-2.5 text-xs text-right text-onSurface-variant">${fmtBytes(p.size)}</td>
      <td class="px-3 py-2.5 text-xs text-right font-mono">${metrics}</td>
      <td class="px-3 py-2.5 text-right whitespace-nowrap">
        <button data-act="edit" data-label="${escapeHtml(p.label)}" class="text-xs text-primary hover:underline">${t('edit')}</button>
        <button data-act="rollback" data-label="${escapeHtml(p.label)}" class="ml-2 text-xs text-tertiary hover:underline">${t('rollback')}</button>
        <button data-act="delete-files" data-label="${escapeHtml(p.label)}" class="ml-2 text-xs text-onSurface-variant hover:text-error hover:underline" title="${t('delFiles')}">${t('delFiles')}</button>
        <button data-act="delete" data-label="${escapeHtml(p.label)}" class="ml-2 text-xs text-error hover:underline">${t('del')}</button>
      </td>
    </tr>`;
  }).join('');
  document.getElementById('historyBody').innerHTML = rows;
  document.querySelectorAll('#historyBody [data-act]').forEach(b => {
    b.onclick = () => handleReleaseAction(b.dataset.act, b.dataset.label);
  });
}
