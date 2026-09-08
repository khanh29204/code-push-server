// ------------------------ Wire up ------------------------

// Mobile sidebar toggle
(function () {
  const sidebar = document.getElementById('sidebar');
  const overlay = document.getElementById('sidebarOverlay');
  document.getElementById('sidebarToggle').addEventListener('click', () => {
    const open = !sidebar.classList.contains('-translate-x-full');
    sidebar.classList.toggle('-translate-x-full', open);
    overlay.classList.toggle('hidden', open);
  });
  overlay.addEventListener('click', () => {
    sidebar.classList.add('-translate-x-full');
    overlay.classList.add('hidden');
  });
  document.getElementById('appList').addEventListener('click', () => {
    if (window.innerWidth < 640) {
      sidebar.classList.add('-translate-x-full');
      overlay.classList.add('hidden');
    }
  });
})();

document.getElementById('logoutBtn').onclick = () => {
  localStorage.removeItem('auth');
  location.href = '/auth/login';
};
document.getElementById('langBtn').onclick = () => setLang(currentLang === 'vi' ? 'en' : 'vi');
document.getElementById('newAppBtn').onclick = createApp;
document.getElementById('renameAppBtn').onclick = renameApp;
document.getElementById('deleteAppBtn').onclick = deleteApp;
document.getElementById('refreshBtn').onclick = () => loadHistory().catch(e => toast(e.message, 'error'));
document.getElementById('historyLimit').onchange = () => loadHistory().catch(e => toast(e.message, 'error'));
document.getElementById('promoteBtn').onclick = promotePackage;
document.getElementById('rollbackLatestBtn').onclick = rollbackLatest;
document.getElementById('clearHistoryBtn').onclick = clearHistory;
document.getElementById('renameDepBtn').onclick = renameDeployment;
document.getElementById('deleteDepBtn').onclick = deleteDeployment;
document.getElementById('copyKeyBtn').onclick = () => {
  const key = document.getElementById('depKey').textContent;
  if (!key) return;
  navigator.clipboard.writeText(key).then(() => toast(t('keyCopied'), 'success'));
};

// Load account info for header badge
fetch('/account', { headers: { Authorization: 'Bearer ' + token() } })
  .then(r => r.ok ? r.json() : null)
  .then(d => {
    if (!d) return;
    const acc = d.account || d;
    const label = acc.email || acc.username || acc.name;
    if (label) {
      const badge = document.getElementById('accountBadge');
      badge.textContent = label;
      badge.classList.remove('hidden');
    }
  }).catch(() => {});

// Bootstrap
setLang(currentLang);
loadApps()
  .then(() => {
    if (state.apps.length && !state.currentApp) {
      return selectApp(state.apps[0].name);
    }
  })
  .catch(e => toast(e.message, 'error'));
