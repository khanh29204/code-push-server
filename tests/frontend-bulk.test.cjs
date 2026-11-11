const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

const root = path.resolve(__dirname, '..');

function frontend(overrides = {}) {
  const context = vm.createContext({
    console,
    setTimeout() {},
    URLSearchParams,
    localStorage: { getItem: () => 'test-token', removeItem() {} },
    location: { href: '' },
    document: { getElementById: (id) => ({ value: '100', id, appendChild() {} }), createElement: () => ({ className: '', textContent: '', remove() {} }) },
    fetch: async () => ({ status: 200, ok: true, text: async () => '{"ok":true}' }),
    toast() {},
    confirm2: async () => true,
    loadHistory: async () => {},
    renderHistory() {},
    renderAppList() {},
    renderDeploymentTabs() {},
    ...overrides,
  });
  for (const file of ['i18n.js', 'utils.js', 'data.js', 'actions.js']) {
    vm.runInContext(fs.readFileSync(path.join(root, 'public/html/js', file), 'utf8'), context, { filename: file });
  }
  const app = vm.runInContext(`({
    state, api, selectDisplayedReleases, pruneSelectedReleases, buildBulkPackageInfo,
    loadHistory: (...args) => loadHistory(...args),
    cleanReleaseFiles: (...args) => cleanReleaseFiles(...args),
    bulkEditSelectedReleases: (...args) => bulkEditSelectedReleases(...args),
    selectApp: (...args) => selectApp(...args),
    createApp: (...args) => createApp(...args),
    captureReleaseTarget, releaseTargetIsCurrent,
    promotePackage: (...args) => promotePackage(...args),
    openReleaseDetail: (...args) => openReleaseDetail(...args),
  })`, context);
  return { ...app, context };
}

test('select-all deduplicates displayed history labels and refresh pruning drops stale labels', () => {
  const app = frontend();
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'v3' }, { label: 'v3' }, { label: 'v2' }];
  app.selectDisplayedReleases(true);
  assert.deepEqual(Array.from(app.state.selectedLabels), ['v3', 'v2']);
  app.state.history = [{ label: 'v3' }, { label: 'v3' }];
  app.pruneSelectedReleases();
  assert.deepEqual(Array.from(app.state.selectedLabels), ['v3']);
});

test('select-all is limited to the displayed release count', () => {
  const app = frontend({ document: { getElementById: () => ({ value: '2' }) } });
  app.state.history = [{ label: 'new', uploadTime: 3 }, { label: 'mid', uploadTime: 2 }, { label: 'old', uploadTime: 1 }];
  app.selectDisplayedReleases(true);
  assert.deepEqual(Array.from(app.state.selectedLabels), ['new', 'mid']);
});

test('refresh prunes labels absent from the newly loaded release history', async () => {
  const calls = [];
  const app = frontend({
    fetch: async (url) => {
      calls.push(url);
      const body = url.endsWith('/history?limit=100') ? { history: [{ label: 'new' }] }
        : url.endsWith('/metrics') ? { metrics: {} }
          : { deployment: { name: 'Production', package: {} } };
      return { status: 200, ok: true, text: async () => JSON.stringify(body) };
    },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'old' }];
  app.state.selectedLabels.add('old');
  app.state.selectionTarget = 'Shop\0Production';
  await app.loadHistory();
  assert.deepEqual(Array.from(app.state.selectedLabels), []);
  assert.equal(calls.filter(url => url.includes('/history?')).length, 1);
  assert.equal(calls.filter(url => url.endsWith('/metrics')).length, 1);
  assert.equal(calls.filter(url => url === '/apps/Shop/deployments/Production').length, 1);
});

test('invalid deployment detail is rejected without replacing loaded history', async () => {
  const messages = [];
  const app = frontend({
    toast: message => messages.push(message),
    fetch: async url => ({ status: 200, ok: true, text: async () => url.endsWith('/history?limit=100') ? '{"history":[{"label":"new"}]}' : url.endsWith('/metrics') ? '{"metrics":{}}' : '{}' }),
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'old' }];
  await assert.rejects(app.loadHistory(), /Invalid deployment details/);
  assert.deepEqual(Array.from(app.state.history, release => release.label), ['old']);
  assert.deepEqual(messages, []);
});

test('stale history responses do not overwrite a newer deployment', async () => {
  let finishHistory;
  const app = frontend({
    fetch: (url) => new Promise(resolve => {
      if (url.includes('/history?')) finishHistory = () => resolve({ status: 200, ok: true, text: async () => '{"history":[{"label":"old"}]}' });
      else resolve({ status: 200, ok: true, text: async () => '{"metrics":{}}' });
    }),
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Old' };
  const load = app.loadHistory();
  app.state.currentDeployment = { name: 'New' };
  app.state.historyLoadGeneration++;
  finishHistory();
  await load;
  assert.deepEqual(Array.from(app.state.history), []);
});

test('selecting an app loads its preferred deployment', async () => {
  const app = frontend({
    fetch: async (url) => ({
      status: 200, ok: true,
      text: async () => url.endsWith('/deployments')
        ? '{"deployments":[{"name":"Staging"},{"name":"Production"}]}'
        : url.endsWith('/history?limit=100') ? '{"history":[]}' : url.endsWith('/metrics') ? '{"metrics":{}}' : '{"deployment":{"name":"Production","package":{"label":"r1"}}}',
    }),
  });
  app.state.apps = [{ name: 'Shop' }];
  await app.selectApp('Shop');
  assert.equal(app.state.currentDeployment?.name, 'Production');
});

test('release target is stale after leaving and returning to an identically named context', () => {
  const app = frontend();
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  const target = app.captureReleaseTarget();
  app.state.contextGeneration++;
  assert.equal(app.releaseTargetIsCurrent(target), false);
});

test('bulk edit posts only chosen settings to the captured release target', async () => {
  const requests = [];
  let renderedBusy;
  let app;
  app = frontend({
    prompt2: async () => ({ isDisabled: 'keep', isMandatory: 'off', rollout: null }),
    confirm2: async () => true,
    loadHistory: async () => { app.state.historyLoadGeneration++; },
    renderHistory() { renderedBusy = app.state.busy; },
    fetch: async (url, options) => {
      requests.push({ url, options });
      return { status: 200, ok: true, text: async () => '{"labels":["r1"]}' };
    },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'r1' }];
  app.state.selectedLabels.add('r1');
  app.state.selectionTarget = 'Shop\0Production';
  await app.bulkEditSelectedReleases();
  const mutations = requests.filter(request => request.options?.method === 'PATCH');
  assert.equal(mutations.length, 1);
  assert.match(mutations[0].url, /\/apps\/Shop\/deployments\/Production\/releases\/bulk$/);
  assert.deepEqual(JSON.parse(mutations[0].options.body), { labels: ['r1'], packageInfo: { isMandatory: false } });
  assert.equal(app.state.busy, false);
  assert.equal(renderedBusy, false);
});

test('bulk rollout confirmation warns that availability may expand', async () => {
  let confirmation = '';
  const app = frontend({
    prompt2: async () => ({ isDisabled: 'keep', isMandatory: 'keep', rollout: 50 }),
    confirm2: async (_title, message) => { confirmation = message; return false; },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'r1' }];
  app.state.selectedLabels.add('r1');
  app.state.selectionTarget = 'Shop\0Production';
  await app.bulkEditSelectedReleases();
  assert.match(confirmation, /may expand availability to more clients/i);
});

test('bulk edit aborts if its target changes while the confirmation is open', async () => {
  const requests = [];
  const app = frontend({
    prompt2: async () => ({ isDisabled: 'on', isMandatory: 'keep', rollout: null }),
    confirm2: async () => { app.state.contextGeneration++; return true; },
    renderHistory() {},
    fetch: async (url, options) => { requests.push({ url, options }); return { status: 200, ok: true, text: async () => '{}' }; },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'r1' }];
  app.state.selectedLabels.add('r1');
  app.state.selectionTarget = 'Shop\0Production';
  await app.bulkEditSelectedReleases();
  assert.equal(requests.length, 0);
});

test('cleanup uses scoped endpoint and retains failed labels without retrying them', async () => {
  const calls = [];
  const messages = [];
  const app = frontend({
    confirm2: async () => true,
    toast: (message, kind) => messages.push(message),
    document: { getElementById: id => id === 'historyLimit' ? { value: '100' } : { appendChild: element => messages.push(element.textContent) }, createElement: () => ({ className: '', textContent: '', remove() {} }) },
    renderHistory() {},
    fetch: async (url, options) => {
      calls.push({ url, options });
      if (url.endsWith('/releases/bulk')) return { status: 200, ok: true, text: async () => JSON.stringify({
        deletedCount: 1, freedBytes: 4096,
        results: [
          { label: 'r1', status: 'cleaned', deletedCount: 1, skippedCount: 0, missingCount: 0, errors: [] },
          { label: 'r2', status: 'skipped', deletedCount: 0, skippedCount: 1, missingCount: 0, errors: ['still referenced'] },
        ],
      }) };
      const body = url.includes('/history?') ? { history: [{ label: 'r1' }, { label: 'r2' }] }
        : url.endsWith('/metrics') ? { metrics: {} }
          : { deployment: { name: 'Production', package: { label: 'r2' } } };
      return { status: 200, ok: true, text: async () => JSON.stringify(body) };
    },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production', package: { label: 'r2' } };
  app.state.deployments = [app.state.currentDeployment];
  app.state.history = [{ label: 'r1' }, { label: 'r2', isDisabled: true }];
  app.state.selectedLabels = new Set(['r1', 'r2']);
  app.state.selectionTarget = 'Shop\0Production';
  await app.cleanReleaseFiles(['r1', 'r2']);
  assert.match(calls[0].url, /\/apps\/Shop\/deployments\/Production\/releases\/bulk$/);
  assert.equal(calls[0].options.method, 'DELETE');
  assert.deepEqual(JSON.parse(calls[0].options.body), { labels: ['r1', 'r2'] });
  const mutations = calls.filter(call => call.options.method === 'DELETE');
  assert.equal(mutations.length, 1);
  assert.deepEqual(JSON.parse(mutations[0].options.body), { labels: ['r1', 'r2'] });
  assert.deepEqual(Array.from(app.state.selectedLabels), ['r2']);
  assert.equal(app.state.cleanupResults.results.find(result => result.label === 'r2').status, 'skipped');
  assert.deepEqual(Array.from(app.state.selectedLabels), ['r2']);

  assert.ok(messages.some(message => message.includes('4.0 KB')));
  assert.equal(calls.some(call => call.url.includes('/storage/audit?label=')), false);
});

test('new app payload uses API platform enum spellings', async () => {
  const requests = [];
  const app = frontend({
    prompt2: async () => ({ name: 'Shop', os: 'iOS', platform: 'React-Native' }),
    fetch: async (url, options) => {
      requests.push({ url, options });
      return { status: 200, ok: true, text: async () => '{"apps":[]}' };
    },
  });
  await app.createApp();
  const create = requests.find(request => request.url === '/apps' && request.options.method === 'POST');
  assert.ok(create);
  assert.deepEqual(JSON.parse(create.options.body), { name: 'Shop', os: 'iOS', platform: 'React-Native' });
});

test('editing a release does not send unchanged appVersion', async () => {
  const calls = [];
  const app = frontend({
    prompt2: async () => ({ description: 'changed', appVersion: '1.0', rollout: 50, isMandatory: true, isDisabled: true }),
    fetch: async (url, options) => { calls.push({ url, options }); return { status: 200, ok: true, text: async () => '{}' }; },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'r1', appVersion: '1.0', description: 'before' }];
  await vm.runInContext("handleReleaseAction('edit', 'r1')", app.context);
  const mutation = calls.find(call => call.options.method === 'PATCH');
  assert.ok(mutation);
  assert.match(mutation.url, /\/apps\/Shop\/deployments\/Production\/release$/);
  assert.deepEqual(JSON.parse(mutation.options.body).packageInfo, { label: 'r1', description: 'changed', rollout: 50, isMandatory: true, isDisabled: true });
});

test('reenabling a single release uses the bulk endpoint artifact checks', async () => {
  const calls = [];
  const app = frontend({
    prompt2: async () => ({ description: 'same', appVersion: '1.0', rollout: 50, isMandatory: true, isDisabled: false }),
    fetch: async (url, options) => { calls.push({ url, options }); return { status: 200, ok: true, text: async () => '{}' }; },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.history = [{ label: 'r1', appVersion: '1.0', description: 'same', rollout: 50, isMandatory: true, isDisabled: true }];
  await vm.runInContext("handleReleaseAction('edit', 'r1')", app.context);
  const mutation = calls.find(call => call.options.method === 'PATCH');
  assert.match(mutation.url, /\/releases\/bulk$/);
  assert.deepEqual(JSON.parse(mutation.options.body), { labels: ['r1'], packageInfo: { isDisabled: false } });
});

test('promote includes a validated rollout in packageInfo and omits an empty rollout', async () => {
  const requests = [];
  const app = frontend({
    prompt2: async () => ({ sourceLabel: '', dest: 'Staging', description: '', appVersion: '', rollout: 25, isMandatory: 'keep', isDisabled: 'keep' }),
    fetch: async (url, options) => { requests.push({ url, options }); return { status: 200, ok: true, text: async () => '{}' }; },
  });
  app.state.currentApp = { name: 'Shop' };
  app.state.currentDeployment = { name: 'Production' };
  app.state.deployments = [{ name: 'Production' }, { name: 'Staging' }];
  await vm.runInContext('promotePackage()', app.context);
  const mutation = requests.find(request => request.options.method === 'POST');
  assert.deepEqual(JSON.parse(mutation.options.body), { packageInfo: { rollout: 25 } });
});

test('API helper rejects HTTP 200 error envelopes', async () => {
  const app = frontend({ fetch: async () => ({ status: 200, ok: true, text: async () => '{"status":"ERROR","message":"not allowed"}' }) });
  await assert.rejects(app.api('GET', '/example'), /not allowed/);
});

test('bulk package payload omits fields left in keep state and preserves explicit false', () => {
  const app = frontend();
  assert.deepEqual(JSON.parse(JSON.stringify(app.buildBulkPackageInfo({ isDisabled: 'keep', isMandatory: 'off', rollout: '' }))), { isMandatory: false });
  assert.deepEqual(JSON.parse(JSON.stringify(app.buildBulkPackageInfo({ isDisabled: 'on', isMandatory: 'keep', rollout: '37' }))), { isDisabled: true, rollout: 37 });
  assert.deepEqual(JSON.parse(JSON.stringify(app.buildBulkPackageInfo({ isDisabled: 'keep', isMandatory: 'keep', rollout: '101' }))), {});
  assert.deepEqual(JSON.parse(JSON.stringify(app.buildBulkPackageInfo({ isDisabled: 'unexpected', isMandatory: null, rollout: null }))), {});
});

test('prompt2 resolves submitted values only after its modal closes', async () => {
  const source = fs.readFileSync(path.join(root, 'public/html/js/modal.js'), 'utf8');
  const promptSource = source.slice(source.indexOf('function prompt2('), source.indexOf('\nfunction confirm2('));
  let actions;
  let closeModal;
  const context = vm.createContext({
    modalSequence: 0,
    document: {
      createElement(tag) {
        if (tag === 'form') return {
          elements: {},
          appendChild(wrap) {
            const name = wrap.innerHTML.match(/name="([^"]+)"/)[1];
            this.elements[name] = { value: '42', checked: false };
          },
          addEventListener() {},
          reportValidity: () => true,
        };
        return { set innerHTML(value) { this.html = value; }, get innerHTML() { return this.html; } };
      },
    },
    escapeHtml: value => String(value),
    t: key => key,
    modal: options => {
      actions = options.actions;
      return new Promise(resolve => { closeModal = resolve; });
    },
  });
  vm.runInContext(promptSource, context);
  let settled = false;
  const result = vm.runInContext("prompt2('title', [{ name: 'rollout', type: 'number' }])", context)
    .then(value => { settled = true; return value; });
  await actions[1].onClick({});
  assert.equal(settled, false);
  closeModal(true);
  assert.deepEqual(JSON.parse(JSON.stringify(await result)), { rollout: 42 });
});

test('prompt2 cancellation resolves null after the modal closes', async () => {
  const source = fs.readFileSync(path.join(root, 'public/html/js/modal.js'), 'utf8');
  const promptSource = source.slice(source.indexOf('function prompt2('), source.indexOf('\nfunction confirm2('));
  let closeModal;
  const context = vm.createContext({
    modalSequence: 0,
    document: { createElement: tag => tag === 'form' ? { appendChild() {}, addEventListener() {} } : {} },
    escapeHtml: value => String(value),
    t: key => key,
    modal: () => new Promise(resolve => { closeModal = resolve; }),
  });
  vm.runInContext(promptSource, context);
  const result = vm.runInContext("prompt2('title', [])", context);
  closeModal(null);
  assert.equal(await result, null);
});

test('prompt2 Enter submits through the footer action but preserves textarea newlines', async () => {
  const source = fs.readFileSync(path.join(root, 'public/html/js/modal.js'), 'utf8');
  const promptSource = source.slice(source.indexOf('function prompt2('), source.indexOf('\nfunction confirm2('));
  const handlers = {};
  let actions;
  let closeModal;
  const form = {
    elements: { rollout: { value: '42' } },
    appendChild() {},
    addEventListener: (name, handler) => { handlers[name] = handler; },
    reportValidity: () => true,
  };
  const context = vm.createContext({
    modalSequence: 0,
    document: {
      createElement: tag => tag === 'form' ? form : {},
      getElementById: () => ({ click: () => actions[1].onClick({}) }),
    },
    escapeHtml: value => String(value),
    t: key => key,
    modal: options => { actions = options.actions; return new Promise(resolve => { closeModal = resolve; }); },
  });
  vm.runInContext(promptSource, context);
  const result = vm.runInContext("prompt2('title', [{ name: 'rollout', type: 'number' }])", context);
  let prevented = false;
  handlers.keydown({ key: 'Enter', target: { tagName: 'INPUT' }, preventDefault() { prevented = true; } });
  assert.equal(prevented, true);
  closeModal(true);
  assert.deepEqual(JSON.parse(JSON.stringify(await result)), { rollout: 42 });

  prevented = false;
  handlers.keydown({ key: 'Enter', target: { tagName: 'TEXTAREA' }, preventDefault() { prevented = true; } });
  assert.equal(prevented, false);
});

test('release details use configured camelCase URLs and localized release metadata', async () => {
  let body;
  const source = fs.readFileSync(path.join(root, 'public/html/js/actions.js'), 'utf8');
  const detailSource = source.slice(source.indexOf('async function openReleaseDetail('), source.indexOf('\nasync function handleReleaseAction('));
  const context = vm.createContext({
    URL,
    location: { origin: 'https://admin.example' },
    state: {
      currentApp: { name: 'Shop' }, currentDeployment: { name: 'Production' }, contextGeneration: 0,
      history: [{
        label: 'r1', appVersion: '1.2', description: 'release note', uploadTime: 1,
        blobUrl: 'https://cdn.example/package.zip', manifestBlobUrl: '/configured/manifest.json',
        diffPackageMap: { 'base-hash': { url: 'https://cdn.example/diff.zip' }, 'bad-hash': { url: 'javascript:alert(1)' } },
        releasedBy: 'release@example.com',
      }],
      metrics: {}, busy: false,
    },
    captureReleaseTarget: () => ({ appName: 'Shop', deploymentName: 'Production', contextGeneration: 0 }),
    releaseTargetIsCurrent: () => true,
    api: async () => ({
      label: 'r1', packageHash: 'package-hash', originalLabel: 'source-r1', originalDeployment: 'Staging', releasedBy: 'release@example.com',
    }),
    escapeHtml: value => String(value),
    t: key => key,
    fmtTime: () => 'time',
    fmtBytes: () => 'size',
    modal: options => { body = options.body; return Promise.resolve(null); },
    toast() {},
  });
  vm.runInContext(detailSource, context);
  await vm.runInContext("openReleaseDetail('r1')", context);
  assert.match(body, /https:\/\/cdn\.example\/package\.zip/);
  assert.match(body, /https:\/\/admin\.example\/configured\/manifest\.json/);
  assert.match(body, /https:\/\/cdn\.example\/diff\.zip/);
  assert.match(body, /source-r1/);
  assert.match(body, /Staging/);
  assert.match(body, /release@example\.com/);
  assert.doesNotMatch(body, /javascript:/);
  assert.doesNotMatch(body, /\/download\/package-hash/);
});
