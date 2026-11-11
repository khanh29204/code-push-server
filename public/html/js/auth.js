// ------------------------ Auth helpers (shared) ------------------------
// Used by login / register / confirm / password / tokens pages.
// Exposes: getAccessToken, ensureLogin, logout, parseQuery, authRequest, authBindForm.

function getAccessToken() {
  return localStorage.getItem('auth');
}

function parseQuery() {
  return Object.fromEntries(new URLSearchParams(location.search));
}

function authShowError(message) {
  var error = document.getElementById('formError');
  if (error) {
    error.textContent = message || '';
    error.hidden = !message;
  }
}

function authBindForm(handler) {
  $('#form').on('submit', function (event) {
    event.preventDefault();
    if (this.dataset.submitting || !this.reportValidity()) return;
    this.dataset.submitting = 'true';
    var button = document.getElementById('submitBtn');
    if (button) button.disabled = true;
    authShowError('');
    var form = this;
    return Promise.resolve().then(function () { return handler(form); })
      .catch(function (error) { authShowError(error.message || 'Request failed.'); })
      .finally(function () {
        form.dataset.submitting = '';
        if (button) button.disabled = false;
      });
  });
}

async function authRequest(url, options) {
  var response = await fetch(url, options || {});
  var data;
  try {
    data = await response.json();
  } catch (error) {
    throw new Error('The server returned an invalid response.');
  }
  if (!response.ok || data.status === 'ERROR') {
    throw new Error(data.message || response.statusText || 'Request failed.');
  }
  return data;
}

function ensureLogin() {
  var token = getAccessToken();
  if (!token) {
    location.href = '/auth/login';
    return Promise.resolve(false);
  }
  return authRequest('/authenticated', { headers: { Authorization: 'Bearer ' + token } })
    .then(function () { return true; })
    .catch(function () {
      localStorage.removeItem('auth');
      if (!location.pathname.startsWith('/auth/login')) location.href = '/auth/login';
      return false;
    });
}

function logout() {
  var token = getAccessToken();
  if (token) {
    fetch('/auth/logout', { method: 'POST', headers: { Authorization: 'Bearer ' + token } }).catch(function () {});
  }
  localStorage.removeItem('auth');
  location.href = '/auth/login';
}
