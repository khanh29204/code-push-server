// ------------------------ Auth helpers (shared) ------------------------
// Used by login / register / confirm / password / tokens pages.
// Exposes: getAccessToken, ensureLogin, logout, parseQuery.

function getAccessToken() {
  return localStorage.getItem('auth');
}

function ensureLogin() {
  if (!getAccessToken()) {
    window.location.href = '/auth/login';
  }
}

function logout() {
  localStorage.removeItem('auth');
  location.href = '/auth/login';
}

function parseQuery() {
  var query = location.search.substring(1);
  var vars = (query || '').split('&');
  var rs = {};
  for (var i = 0; i < vars.length; i++) {
    if (!vars[i]) continue;
    var pair = vars[i].split('=');
    rs[decodeURIComponent(pair[0])] = decodeURIComponent(pair[1] || '');
  }
  return rs;
}
