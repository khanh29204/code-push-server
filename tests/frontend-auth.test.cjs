const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const root = path.resolve(__dirname, '..');

class Element {
  constructor(id) {
    this.id = id;
    this.value = '';
    this.disabled = false;
    this.hidden = id === 'formError';
    this.style = {};
    this.textContent = '';
    this.listeners = {};
    this.attributes = {};
    this.classList = { add() {}, remove() {} };
  }

  addEventListener(type, listener) {
    (this.listeners[type] ||= []).push(listener);
  }

  reportValidity() { return true; }

  async dispatch(type) {
    const event = { preventDefault() { this.defaultPrevented = true; } };
    for (const listener of this.listeners[type] || []) await listener.call(this, event);
    return event;
  }
}

function jsonResponse(body, status = 200) {
  return { ok: status >= 200 && status < 300, status, json: async () => body };
}

function loadPage(files, { query = '', token = null, fetchImpl } = {}) {
  const ids = ['form', 'submitBtn', 'inputEmail', 'inputPassword', 'inputToken', 'inputNewPassword', 'formError', 'registerBtn', 'myModal', 'okBtn', 'generateSection', 'tipsSuccess', 'key', 'copyBtn', 'copyLabel', 'tipsClose'];
  const elements = Object.fromEntries(ids.map(id => [id, new Element(id)]));
  elements.inputEmail.value = 'person@example.test';
  elements.inputEmail.type = 'email';
  elements.inputPassword.value = 'old-pass';
  elements.inputPassword.type = 'password';
  elements.inputToken.value = 'verification-code';
  elements.inputToken.type = 'text';
  elements.inputNewPassword.value = 'new-pass';
  elements.inputNewPassword.type = 'password';
  elements.form.fields = ['account', 'password', 'email', 'token', 'oldPassword', 'newPassword'];
  elements.form.attributes.action = '/auth/login';
  elements.form.action = '/auth/login';
  elements.form.dataset = {};
  elements.form.elements = Object.fromEntries(elements.form.fields.map(name => [name, elements[
    { account: 'inputEmail', password: 'inputPassword', email: 'inputEmail', token: 'inputToken', oldPassword: 'inputPassword', newPassword: 'inputNewPassword' }[name]
  ]]));
  elements.form.reportValidity = () => {
    for (const input of Object.values(elements.form.elements)) {
      if (input.type === 'email' && !/^[^@]+@[^@]+\.[^@]+$/.test(input.value)) return false;
      const minlength = Number(input.attributes.minlength || 0);
      const maxlength = Number(input.attributes.maxlength || Infinity);
      if ((minlength && input.value.length < minlength) || input.value.length > maxlength) return false;
    }
    return true;
  };
  const ajaxCalls = [];
  const fetchCalls = [];
  const storage = new Map(token ? [['auth', token]] : []);
  const location = { search: query, pathname: '', href: '' };

  function $(selector) {
    const id = typeof selector === 'string' && selector.startsWith('#') ? selector.slice(1) : selector;
    const element = elements[id] || new Element(id);
    return {
      on(type, callback) { element.addEventListener(type, callback); return this; },
      val(value) { if (value === undefined) return element.value; element.value = value; return this; },
      attr(name) { return element.attributes[name]; },
      serializeArray() { return element.fields.map(name => ({ name, value: elements['input' + name[0].toUpperCase() + name.slice(1)]?.value || '' })); },
      removeClass() { return this; },
      addClass() { return this; },
      hide() { element.hidden = true; return this; },
      show() { element.hidden = false; return this; },
      text(value) { element.textContent = value; return this; }
    };
  }
  $.ajax = options => { ajaxCalls.push(options); };

  const context = {
    $, document: { getElementById: id => elements[id], querySelectorAll: () => [] },
    window: { location, localStorage: null }, location,
    localStorage: { getItem: key => storage.get(key) ?? null, setItem: (key, value) => storage.set(key, value), removeItem: key => storage.delete(key) },
    navigator: { language: 'en', userLanguage: 'en', clipboard: { writeText: async () => {} } },
    URLSearchParams, URL, console: { log() {}, error() {} }, setTimeout,
    fetch: async (url, options = {}) => {
      fetchCalls.push({ url, options });
      return fetchImpl ? fetchImpl(url, options) : { ok: true, status: 200, json: async () => ({ status: 'OK' }) };
    },
    alert() {},
    documentExecCommandResult: true
  };
  context.$ = selector => selector === '[data-dismiss="modal"]' ? { on() { return this; } } : $(selector);
  context.$.ajax = $.ajax;
  context.window.localStorage = context.localStorage;
  context.document.execCommand = () => context.documentExecCommandResult;
  vm.createContext(context);
  for (const file of files) {
    vm.runInContext(fs.readFileSync(path.join(root, file), 'utf8'), context, { filename: file });
  }
  return { context, elements, ajaxCalls, fetchCalls, storage, location };
}

test('registration code form submits JSON to the JSON-only endpoint', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/register.js']);

  await page.elements.form.dispatch('submit');

  assert.equal(page.fetchCalls.length, 1);
  assert.equal(page.fetchCalls[0].url, '/users/registerCode');
  assert.equal(page.fetchCalls[0].options.headers['Content-Type'], 'application/json');
  assert.deepEqual(JSON.parse(page.fetchCalls[0].options.body), { email: 'person@example.test' });
});

test('confirmation submits JSON and reports business errors returned with HTTP 200', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/confirm.js'], {
    fetchImpl: async () => jsonResponse({ status: 'ERROR', message: 'Code expired' })
  });

  await page.elements.form.dispatch('submit');

  assert.equal(page.fetchCalls[0].url, '/users/');
  assert.equal(page.fetchCalls[0].options.headers['Content-Type'], 'application/json');
  assert.deepEqual(JSON.parse(page.fetchCalls[0].options.body), {
    email: 'person@example.test', token: 'verification-code', password: 'old-pass'
  });
  assert.equal(page.elements.formError.textContent, 'Code expired');
  assert.equal(page.elements.submitBtn.disabled, false);
  assert.equal(page.location.href, '');
  assert.doesNotMatch(fs.readFileSync(path.join(root, 'public/html/js/confirm.js'), 'utf8'), /console\.log/);
});

test('invalid saved session is cleared before redirecting protected pages', async () => {
  const page = loadPage(['public/html/js/auth.js'], {
    token: 'stale-token',
    fetchImpl: async () => jsonResponse({ status: 'ERROR', message: 'invalid' })
  });

  page.context.ensureLogin();
  await new Promise(resolve => setImmediate(resolve));

  assert.equal(page.fetchCalls[0].url, '/authenticated');
  assert.equal(page.storage.has('auth'), false);
  assert.equal(page.location.href, '/auth/login');
});

test('ensureLogin does not redirect to login page when its stored token is stale', async () => {
  const page = loadPage(['public/html/js/auth.js'], {
    token: 'stale-token',
    fetchImpl: async () => jsonResponse({ status: 'ERROR', message: 'invalid' })
  });
  page.location.pathname = '/auth/login';

  await page.context.ensureLogin();

  assert.equal(page.storage.has('auth'), false);
  assert.equal(page.location.href, '');
});

test('token creation sends string millisecond TTL and CLI hostname without session flag', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/tokens.js'], {
    token: 'valid-token',
    query: '?hostname=cli.example',
    fetchImpl: async url => url === '/authenticated'
      ? jsonResponse({ authenticated: true })
      : jsonResponse({ accessKey: { name: 'one-time-token' } })
  });

  await page.elements.submitBtn.dispatch('click');
  await new Promise(resolve => setImmediate(resolve));

  const call = page.fetchCalls.find(request => request.url === '/accessKeys');
  assert.ok(call);
  const payload = JSON.parse(call.options.body);
  assert.equal(payload.createdBy, 'cli.example');
  assert.match(payload.friendlyName, /^Login-\d+$/);
  assert.equal(payload.description, payload.friendlyName);
  assert.equal(payload.ttl, '2592000000');
  assert.equal('isSession' in payload, false);
  assert.equal(page.elements.key.value, 'one-time-token');
  assert.equal(page.storage.has('auth'), true);
  assert.equal(page.elements.submitBtn.disabled, false);
});

test('token API error never reveals a false token result', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/tokens.js'], {
    token: 'valid-token',
    fetchImpl: async url => url === '/authenticated'
      ? jsonResponse({ authenticated: true })
      : jsonResponse({ status: 'ERROR', message: 'Unauthorized' })
  });

  await page.elements.submitBtn.dispatch('click');
  await new Promise(resolve => setImmediate(resolve));

  assert.equal(page.elements.formError.textContent, 'Unauthorized');
  assert.equal(page.elements.key.value, '');
  assert.equal(page.elements.generateSection.hidden, false);
});

test('token generation stays single-flight while session validation is pending', async () => {
  let resolveValidation;
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/tokens.js'], {
    token: 'valid-token',
    fetchImpl: url => url === '/authenticated'
      ? new Promise(resolve => { resolveValidation = resolve; })
      : jsonResponse({ accessKey: { name: 'one-time-token' } })
  });
  const first = page.elements.submitBtn.dispatch('click');
  const second = page.elements.submitBtn.dispatch('click');
  assert.equal(page.elements.submitBtn.disabled, true);
  resolveValidation(jsonResponse({ authenticated: true }));
  await Promise.all([first, second]);

  assert.equal(page.fetchCalls.filter(request => request.url === '/accessKeys').length, 1);
});

test('login submits JSON, shows body errors and re-enables after failure', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/login.js'], {
    fetchImpl: async () => jsonResponse({ status: 'ERROR', message: 'Bad password' })
  });

  await page.elements.form.dispatch('submit');

  const call = page.fetchCalls.find(request => request.url === '/auth/login');
  assert.deepEqual(JSON.parse(call.options.body), { account: 'person@example.test', password: 'old-pass' });
  assert.equal(page.elements.formError.textContent, 'Bad password');
  assert.equal(page.elements.submitBtn.disabled, false);
  assert.equal(page.storage.has('auth'), false);
});

test('network and HTTP errors are visible instead of swallowed', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/register.js'], {
    fetchImpl: async () => jsonResponse({ message: 'Unavailable' }, 503)
  });

  await page.elements.form.dispatch('submit');

  assert.equal(page.elements.formError.textContent, 'Unavailable');
  assert.equal(page.elements.submitBtn.disabled, false);
});

test('network rejection produces visible feedback and permits retry', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/register.js'], {
    fetchImpl: async () => { throw new Error('Network disconnected'); }
  });

  await page.elements.form.dispatch('submit');

  assert.equal(page.elements.formError.textContent, 'Network disconnected');
  assert.equal(page.elements.submitBtn.disabled, false);
  assert.equal(page.elements.form.dataset.submitting, '');
});

test('invalid fields block JSON requests from submit handlers', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/register.js']);
  page.elements.inputEmail.value = 'not-an-email';

  await page.elements.form.dispatch('submit');

  assert.equal(page.fetchCalls.length, 0);
  assert.equal(page.elements.submitBtn.disabled, false);
});

test('password change reports errors and logs out locally only after success', async () => {
  const failing = loadPage(['public/html/js/auth.js', 'public/html/js/password.js'], {
    token: 'valid-token',
    fetchImpl: async url => url === '/authenticated'
      ? jsonResponse({ authenticated: true })
      : jsonResponse({ status: 'ERROR', message: 'Wrong old password' })
  });
  await failing.elements.form.dispatch('submit');
  assert.equal(failing.elements.formError.textContent, 'Wrong old password');
  assert.equal(failing.storage.has('auth'), true);

  const succeeding = loadPage(['public/html/js/auth.js', 'public/html/js/password.js'], {
    token: 'valid-token',
    fetchImpl: async url => url === '/authenticated'
      ? jsonResponse({ authenticated: true })
      : jsonResponse({ status: 'OK' })
  });
  await succeeding.elements.form.dispatch('submit');
  assert.equal(succeeding.storage.has('auth'), false);
  assert.equal(succeeding.location.href, '/auth/login');
});

test('register submit button and all requests are protected against duplicate form submits', async () => {
  let resolveRequest;
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/register.js'], {
    fetchImpl: () => new Promise(resolve => { resolveRequest = resolve; })
  });
  const first = page.elements.form.dispatch('submit');
  const second = page.elements.form.dispatch('submit');
  await Promise.resolve();

  assert.equal(page.fetchCalls.filter(request => request.url === '/users/registerCode').length, 1);
  assert.equal(page.elements.submitBtn.disabled, true);
  resolveRequest(jsonResponse({ status: 'OK' }));
  await Promise.all([first, second]);
  assert.equal(page.elements.submitBtn.disabled, false);
});

test('login validates stored session without redirect loop for an expired token', async () => {
  const page = loadPage(['public/html/js/auth.js', 'public/html/js/login.js'], {
    token: 'expired-token',
    fetchImpl: async () => jsonResponse({ status: 'ERROR', message: 'invalid' })
  });

  await new Promise(resolve => setImmediate(resolve));
  assert.equal(page.storage.has('auth'), false);
  assert.equal(page.location.href, '');
});

test('form submit controls use native button semantics and accessible inline errors', () => {
  for (const file of ['login', 'register', 'confirm', 'password']) {
    const html = fs.readFileSync(path.join(root, `public/html/${file}.html`), 'utf8');
    assert.match(html, /<button[^>]*id="submitBtn"[^>]*type="submit"|<button[^>]*type="submit"[^>]*id="submitBtn"/);
    assert.match(html, /id="formError"[^>]*aria-live="polite"/);
  }
});

test('register and confirm email/password fields enforce backend validation before submit', () => {
  for (const file of ['register', 'confirm']) {
    const html = fs.readFileSync(path.join(root, `public/html/${file}.html`), 'utf8');
    assert.match(html, /type="email" name="email"[^>]*required/);
    if (file === 'confirm') assert.match(html, /name="password"[^>]*minlength="6"/);
  }
});

test('query parsing preserves encoded delimiters and decodes plus characters', () => {
  const page = loadPage(['public/html/js/auth.js']);
  page.location.search = '?hostname=cli%26safe%3Dyes&email=a%2Bb%40example.test';
  assert.deepEqual(JSON.parse(JSON.stringify(page.context.parseQuery())), {
    hostname: 'cli&safe=yes', email: 'a+b@example.test'
  });
});
