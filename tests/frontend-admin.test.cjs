const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const vm = require("node:vm");
const path = require("node:path");

const root = path.resolve(__dirname, "..");

function frontend(overrides = {}) {
  const calls = [];
  const dialogs = [];
  const values = {};
  const toasts = [];
  const formChecks = [];
  const result = { innerHTML: "", textContent: "" };
  const state = {
    currentApp: null,
    currentDeployment: null,
    apps: [],
    deployments: [],
    history: [],
    metrics: {},
    selectedLabels: new Set(),
    selectionTarget: null,
  };
  const context = vm.createContext({
    URLSearchParams,
    Date,
    currentLang: "en",
    escapeHtml: (value) =>
      String(value).replace(
        /[&<>"']/g,
        (character) =>
          ({
            "&": "&amp;",
            "<": "&lt;",
            ">": "&gt;",
            '"': "&quot;",
            "'": "&#39;",
          })[character],
      ),
    state,
    document: {
      getElementById: () => ({ value: "100" }),
      createElement: () => {
        const listeners = {};
        return {
          innerHTML: "",
          listeners,
          addEventListener(name, handler) {
            listeners[name] = handler;
          },
          querySelector(selector) {
            if (selector === "[data-result]") return result;
            if (selector === '[name="protocol"]') return { value: values.protocol || "current" };
            if (selector.startsWith("[data-admin-form=")) {
              return {
                querySelector(inputSelector) {
                  const inputName = inputSelector.slice(7, -2);
                  return { value: values[inputName] || "" };
                },
                reportValidity() {
                  formChecks.push(selector);
                  return overrides.isFormValid ? overrides.isFormValid(selector, values) : true;
                },
              };
            }
            const name = selector.startsWith('[name="') ? selector.slice(7, -2) : null;
            return name ? { value: values[name] || "" } : null;
          },
        };
      },
    },
    api: async (...args) => {
      calls.push(args);
      return {};
    },
    modal: async (options) => {
      dialogs.push(options);
      return null;
    },
    toast(...args) {
      toasts.push(args);
    },
    confirm2: async () => true,
    loadApps: async () => {},
    selectApp: async () => {},
    clearSelectedReleases() {
      state.selectedLabels.clear();
    },
    renderDeploymentTabs() {},
    renderHistory() {},
    navigator: { clipboard: { writeText: async () => {} } },
    ...overrides,
  });
  vm.runInContext(fs.readFileSync(path.join(root, "public/html/js/admin.js"), "utf8"), context, {
    filename: "admin.js",
  });
  return { context, calls, dialogs, values, result, toasts, formChecks, state };
}

test("revoking a listed access key uses its friendly name, not the hidden token field", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url === "/accessKeys") {
        return {
          accessKeys: [
            {
              friendlyName: "release key",
              name: "(hidden)",
              expires: Date.now() + 10000,
            },
          ],
        };
      }
      if (url === "/account") return { account: { email: "admin@example.test" } };
      return {};
    },
  });
  await vm.runInContext("openAccessKeys()", app.context);
  await app.dialogs[0].body.listeners.click({
    target: {
      closest: () => ({
        dataset: { adminAction: "revoke-access-key", index: "0" },
      }),
    },
  });
  assert.deepEqual(app.calls.at(-1), ["DELETE", "/accessKeys/release%20key"]);
});

test("creating access keys validates and sends TTL as a millisecond string", async () => {
  const app = frontend({
    api: async (method, url, body) => {
      app.calls.push([method, url, body]);
      if (url === "/accessKeys" && method === "GET") return { accessKeys: [] };
      if (url === "/account") return { account: { email: "admin@example.test" } };
      if (method === "POST") return { accessKey: { name: "secret-once" } };
      return {};
    },
    modal: async (options) => {
      app.dialogs.push(options);
      if (!app.created && options.title === "Access keys" && options.actions.length === 2) {
        app.created = true;
        await options.actions[1].onClick(options.body);
      }
      return null;
    },
  });
  app.values.friendlyName = "CI";
  app.values.ttlDays = "14";
  app.values.description = "automation";
  await vm.runInContext("openAccessKeys()", app.context);
  const request = app.calls.find(([method, url]) => method === "POST" && url === "/accessKeys");
  assert.equal(
    JSON.stringify(request[2]),
    JSON.stringify({
      createdBy: "admin@example.test",
      friendlyName: "CI",
      ttl: "1209600000",
      description: "automation",
    }),
  );
  assert.equal(
    app.calls.filter(([method, url]) => method === "POST" && url === "/accessKeys").length,
    1,
  );
  assert.equal(app.dialogs.length, 2);
  assert.equal(app.dialogs[0].actions.length, 2);
  assert.equal(app.dialogs[1].actions.length, 2);
  assert.match(app.dialogs.find((dialog) => typeof dialog.body === "string").body, /secret-once/);
});

test("access-key TTL outside positive bounded range is rejected before API call", async () => {
  const app = frontend({
    api: async (...args) => {
      app.calls.push(args);
      return args[1] === "/accessKeys"
        ? { accessKeys: [] }
        : { account: { email: "owner@example.test" } };
    },
  });
  app.values.friendlyName = "CI";
  app.values.ttlDays = "0";
  await vm.runInContext("openAccessKeys()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.equal(
    app.calls.some(([method, url]) => method === "POST" && url === "/accessKeys"),
    false,
  );
  assert.ok(app.toasts.some(([message]) => String(message).includes("lifetime")));
});

test("access-key lifetime must be a whole number of days", async () => {
  const app = frontend({
    api: async (...args) => {
      app.calls.push(args);
      return args[1] === "/accessKeys"
        ? { accessKeys: [] }
        : { account: { email: "owner@example.test" } };
    },
    isFormValid: () => true,
  });
  app.values.friendlyName = "CI";
  app.values.ttlDays = "0.1";
  await vm.runInContext("openAccessKeys()", app.context);
  app.isFormValid = (selector) => selector !== '[data-admin-form="access-key"]';
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.equal(
    app.calls.some(([method, url]) => method === "POST" && url === "/accessKeys"),
    false,
  );
  assert.ok(app.formChecks.includes('[data-admin-form="access-key"]'));
});

test("collaborator route encodes app name and email path segments", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      return method === "GET" ? { collaborators: {} } : {};
    },
  });
  app.values.email = "dev+ci@example.test";
  app.isFormValid = () => true;
  vm.runInContext(
    "state.currentApp = {name:'My App',collaborators:{'owner@example.test':{permission:'Owner',isCurrentAccount:true}}}",
    app.context,
  );
  await vm.runInContext("openAppSettings()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.ok(
    app.calls.some(
      ([method, url]) =>
        method === "POST" && url === "/apps/My%20App/collaborators/dev%2Bci%40example.test",
    ),
  );
});

test("collaborator email uses native email validation without requiring transfer fields", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      return method === "GET" ? { collaborators: {} } : {};
    },
    isFormValid: (selector, values) =>
      selector !== '[data-admin-form="add-collaborator"]' ||
      /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(values.email || ""),
  });
  app.values.email = "invalid-email";
  vm.runInContext(
    "state.currentApp = {name:'My App',collaborators:{'owner@example.test':{permission:'Owner',isCurrentAccount:true}}}",
    app.context,
  );
  await vm.runInContext("openAppSettings()", app.context);
  const body = app.dialogs[0].body.innerHTML;
  assert.match(body, /data-admin-form="add-collaborator"/);
  assert.match(body, /name="email"[^>]*type="email"/);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.ok(app.formChecks.includes('[data-admin-form="add-collaborator"]'));
  assert.equal(
    app.calls.some(([method]) => method === "POST"),
    false,
  );
});

test("collaborator row removal lives in the scrollable body and not the footer", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      return method === "GET"
        ? {
            collaborators: {
              "owner@example.test": { permission: "Owner" },
              "dev@example.test": { permission: "Collaborator" },
            },
          }
        : {};
    },
  });
  vm.runInContext(
    "state.currentApp = {name:'My App',collaborators:{'owner@example.test':{permission:'Owner',isCurrentAccount:true}}}",
    app.context,
  );
  await vm.runInContext("openAppSettings()", app.context);
  const dialog = app.dialogs[0];
  assert.equal(dialog.actions.length, 3);
  assert.match(dialog.body.innerHTML, /data-admin-action="remove-collaborator"/);
  await dialog.body.listeners.click({
    target: {
      closest: () => ({
        dataset: { adminAction: "remove-collaborator", index: "1" },
      }),
    },
  });
  assert.ok(
    app.calls.some(([method, url]) => method === "DELETE" && url.endsWith("/dev%40example.test")),
  );
});

test("collaborator removal refreshes current app metadata", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url.includes("/collaborators")) {
        return {
          collaborators: {
            "owner@example.test": { permission: "Owner" },
            "dev@example.test": { permission: "Collaborator" },
          },
        };
      }
      if (url === "/apps") {
        return {
          apps: [
            {
              name: "My App",
              collaborators: {
                "owner@example.test": { permission: "Owner", isCurrentAccount: true },
              },
            },
          ],
        };
      }
      return {};
    },
  });
  vm.runInContext(
    "state.currentApp = {name:'My App',collaborators:{'owner@example.test':{permission:'Owner',isCurrentAccount:true},'dev@example.test':{permission:'Collaborator'}}}",

    app.context,
  );
  await vm.runInContext("openAppSettings()", app.context);
  await app.dialogs[0].body.listeners.click({
    target: { closest: () => ({ dataset: { adminAction: "remove-collaborator", index: "1" } }) },
  });
  assert.equal(app.state.currentApp.collaborators["owner@example.test"].permission, "Owner");
  assert.ok(
    app.calls.some(([method, url]) => method === "DELETE" && url.endsWith("/dev%40example.test")),
  );
});

test("access-key revoke controls live in the scrollable body and footer stays bounded", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url === "/apps") return { apps: [] };
      if (url === "/accessKeys") {
        return {
          accessKeys: Array.from({ length: 105 }, (_, index) => ({
            friendlyName: `key-${index}`,
            expires: Date.now() + 10000,
          })),
        };
      }
      if (url === "/account") return { account: { email: "admin@example.test" } };
      return {};
    },
  });
  await vm.runInContext("openAccessKeys()", app.context);
  assert.equal(app.dialogs[0].actions.length, 2);
  assert.match(app.dialogs[0].body.innerHTML, /data-admin-action="revoke-access-key"/);
  assert.match(app.dialogs[0].body.innerHTML, /Showing the first 100 entries/);
});

test("transfer does not reset app state or selections after switching apps while pending", async () => {
  let finishTransfer;
  let selectedAppCalls = 0;
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url.includes("/collaborators")) return { collaborators: {} };
      if (url.includes("/transfer/"))
        return new Promise((resolve) => {
          finishTransfer = resolve;
        });
      if (url === "/apps") return { apps: [] };
      return {};
    },
    selectApp: async () => {
      selectedAppCalls += 1;
    },
  });
  app.values.email = "new-owner@example.test";
  app.values.appName = "App A";
  app.isFormValid = () => true;
  vm.runInContext(
    "state.currentApp = {name:'App A',collaborators:{'owner@example.test':{permission:'Owner',isCurrentAccount:true}}}; state.currentDeployment={name:'Production'}; state.selectedLabels=new Set(['keep']); state.selectionTarget='App B\\0Production'",
    app.context,
  );
  await vm.runInContext("openAppSettings()", app.context);
  const pending = app.dialogs[0].actions[2].onClick(app.dialogs[0].body);
  await new Promise((resolve) => setImmediate(resolve));
  vm.runInContext(
    "state.currentApp={name:'App B'}; state.currentDeployment={name:'Staging'}; state.selectedLabels=new Set(['other']); state.selectionTarget='App B\\0Staging'",
    app.context,
  );
  finishTransfer({});
  await pending;
  assert.equal(app.state.currentApp.name, "App B");
  assert.equal(app.state.currentDeployment.name, "Staging");
  assert.deepEqual([...app.state.selectedLabels], ["other"]);
  assert.equal(selectedAppCalls, 0);
});

test("diagnostics use selected update-check protocol and encoded query values", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url === "/health") return "OK";
      if (url === "/config") return { allowRegistration: false };
      return { updateInfo: { isAvailable: false } };
    },
  });
  vm.runInContext("state.currentDeployment = {key:'key&value'}", app.context);
  Object.assign(app.values, {
    deploymentKey: "key&value",
    appVersion: "1.2 beta",
    label: "v+2",
    protocol: "legacy",
  });
  await vm.runInContext("openDiagnostics()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  const request = app.calls.find(([, url]) =>
    url.startsWith("/v0.1/public/codepush/update_check?"),
  );
  const query = new URL(request[1], "http://localhost").searchParams;
  assert.equal(query.get("deployment_key"), "key&value");
  assert.equal(query.get("app_version"), "1.2 beta");
  assert.equal(query.get("label"), "v+2");
  assert.equal(query.has("deploymentKey"), false);
});

test("legacy diagnostics recognize the authentic snake_case available response", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      if (url === "/health") return "OK";
      if (url === "/config") return {};
      return { update_info: { is_available: true } };
    },
  });

  app.values.deploymentKey = "deploy-key";
  app.values.appVersion = "1.0";
  app.values.protocol = "legacy";
  await vm.runInContext("openDiagnostics()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.match(app.result.innerHTML, /Update available/);
});

test("diagnostics also supports server camelCase updateCheck fields", async () => {
  const app = frontend({
    api: async (method, url) => {
      app.calls.push([method, url]);
      return url === "/health"
        ? "OK"
        : url === "/config"
          ? {}
          : { updateInfo: { isAvailable: true } };
    },
  });
  app.values.deploymentKey = "deploy-key";
  app.values.appVersion = "1.0";
  app.values.protocol = "current";
  await vm.runInContext("openDiagnostics()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  const request = app.calls.find(([, url]) => url.startsWith("/updateCheck?"));
  const query = new URL(request[1], "http://localhost").searchParams;
  assert.equal(query.get("deploymentKey"), "deploy-key");
  assert.equal(query.get("appVersion"), "1.0");
  assert.match(app.result.innerHTML, /Update available/);
});

test("access-key validation message follows the selected language", async () => {
  const app = frontend({
    currentLang: "vi",
    isFormValid: () => true,
    api: async (...args) => {
      app.calls.push(args);
      return args[1] === "/accessKeys"
        ? { accessKeys: [] }
        : { account: { email: "admin@example.test" } };
    },
  });
  app.values.friendlyName = "CI";
  app.values.ttlDays = "0.5";
  await vm.runInContext("openAccessKeys()", app.context);
  await app.dialogs[0].actions[1].onClick(app.dialogs[0].body);
  assert.ok(
    app.toasts.some(
      ([message]) => message === "Nhập tên và thời hạn là số nguyên từ 1 đến 3650 ngày.",
    ),
  );
  assert.equal(
    app.calls.some(([method, url]) => method === "POST" && url === "/accessKeys"),
    false,
  );
});

test("storage audit failure is reported instead of showing a fabricated report", async () => {
  const app = frontend({
    api: async (...args) => {
      app.calls.push(args);
      throw new Error("audit unavailable");
    },
  });
  await vm.runInContext("openStorageAudit()", app.context);
  assert.equal(app.dialogs.length, 0);
  assert.ok(app.toasts.some(([message]) => message === "audit unavailable"));
});

test("storage audit warns global scope and exposes no cleanup request", async () => {
  const app = frontend({
    api: async (...args) => {
      app.calls.push(args);
      return {
        summary: {},
        orphanedFiles: [],
        missingFiles: [],
        validFiles: [],
      };
    },
  });
  await vm.runInContext("openStorageAudit()", app.context);
  assert.match(app.dialogs[0].body.innerHTML, /Global audit/);
  assert.match(app.dialogs[0].body.innerHTML, /disabled/);
  assert.match(app.dialogs[0].body.innerHTML, /packages_diff references/);
  assert.deepEqual(
    app.calls.map(([method, url]) => [method, url]),
    [["GET", "/storage/audit"]],
  );
});
