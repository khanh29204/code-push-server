// Admin panels use the shared api/modal helpers from utils.js and modal.js.
const adminText = {
  en: {
    settings: "App settings",
    collaborators: "Collaborators",
    ownerOnly:
      "Only owners can manage collaborators. This account is a collaborator; the backend does not allow collaborator management.",
    email: "Email",
    permission: "Permission",
    add: "Add collaborator",
    addAction: "Add",
    remove: "Remove",
    transfer: "Transfer ownership",
    transferAction: "Transfer",
    appName: "Type app name to confirm",
    close: "Close",
    keys: "Access keys",
    keyName: "Key name",
    createdBy: "Created by",
    description: "Description",
    created: "Created",
    expires: "Expires",
    ttl: "Lifetime (days)",
    createKey: "Create access key",
    revoke: "Revoke",
    expired: "Expired",
    copy: "Copy token",
    copied: "Token copied",
    tokenOnce: "Copy this access key now. It is only displayed once.",
    account: "Account",
    storage: "Storage audit",
    globalWarning:
      "Global audit: this report covers files for the entire server, not only the selected app. Paths and labels may be sensitive.",
    orphaned: "Orphaned files",
    missing: "Missing files",
    valid: "Valid files",
    hash: "Hash",
    size: "Size",
    modified: "Modified",
    path: "Path",
    expectedPath: "Expected path",
    label: "Label",
    type: "Type",
    listLimit: "Showing the first 100 entries.",
    cleanup: "Cleanup orphaned files",
    cleanupReason:
      "Cleanup is disabled until backend protection for packages_diff references and authorization for this global operation are verified.",
    diagnostics: "Diagnostics",
    health: "Health",
    config: "Configuration",
    check: "Check update",
    deploymentKey: "Deployment key",
    appVersion: "App version",
    packageHash: "Package hash",
    clientUniqueId: "Client unique ID",
    protocol: "Protocol",
    current: "Server updateCheck (camelCase)",
    legacy: "CodePush v0.1 (snake_case)",
    cacheWarning:
      "Update checks may affect cached client rollout decisions. This panel does not report download or deployment status.",
    available: "Update available",
    unavailable: "No update available",
    noData: "None",
    error: "Request failed",
    requiredEmail: "Enter a valid email address.",
    invalidKey: "Enter a name and a whole-number lifetime between 1 and 3650 days.",
    transferInvalid: "Confirm the exact app name and enter a different existing account email.",
    diagnosticsRequired: "Deployment key and app version are required.",
    addSucceeded: "Collaborator added",
    removed: "Collaborator removed",
    transferred: "Ownership transferred",
    keyRevoked: "Access key revoked",
    keyCreated: "Access key created",
    confirmTransfer: (app, email) => `Transfer ${app} to ${email}?`,
    confirmRemove: (email) => `Remove ${email}?`,
    confirmRevoke: (name) => `Revoke ${name}?`,
  },
  vi: {
    settings: "Cài đặt ứng dụng",
    collaborators: "Cộng tác viên",
    ownerOnly:
      "Chỉ chủ sở hữu có thể quản lý cộng tác viên. Tài khoản này là cộng tác viên; backend không cho phép quản lý cộng tác viên.",
    email: "Email",
    permission: "Quyền",
    add: "Thêm cộng tác viên",
    addAction: "Thêm",
    remove: "Gỡ",
    transfer: "Chuyển quyền sở hữu",
    transferAction: "Chuyển",
    appName: "Nhập tên ứng dụng để xác nhận",
    close: "Đóng",
    keys: "Access key",
    keyName: "Tên key",
    createdBy: "Người tạo",
    description: "Mô tả",
    created: "Ngày tạo",
    expires: "Hết hạn",
    ttl: "Thời hạn (ngày)",
    createKey: "Tạo access key",
    revoke: "Thu hồi",
    expired: "Đã hết hạn",
    copy: "Sao chép token",
    copied: "Đã sao chép token",
    tokenOnce: "Hãy sao chép access key ngay. Key chỉ hiển thị một lần.",
    account: "Tài khoản",
    storage: "Audit lưu trữ",
    globalWarning:
      "Audit toàn cục: báo cáo bao gồm file của toàn máy chủ, không chỉ ứng dụng đang chọn. Đường dẫn và nhãn có thể nhạy cảm.",
    orphaned: "File mồ côi",
    missing: "File bị thiếu",
    valid: "File hợp lệ",
    hash: "Hash",
    size: "Dung lượng",
    modified: "Ngày sửa",
    path: "Đường dẫn",
    expectedPath: "Đường dẫn dự kiến",
    label: "Nhãn",
    type: "Loại",
    listLimit: "Chỉ hiển thị 100 mục đầu.",
    cleanup: "Dọn file mồ côi",
    cleanupReason:
      "Đã tắt dọn file cho đến khi xác minh bảo vệ tham chiếu packages_diff và quyền truy cập thao tác toàn cục này ở backend.",
    diagnostics: "Chẩn đoán",
    health: "Tình trạng",
    config: "Cấu hình",
    check: "Kiểm tra cập nhật",
    deploymentKey: "Deployment key",
    appVersion: "Phiên bản app",
    packageHash: "Hash gói",
    clientUniqueId: "ID client",
    protocol: "Giao thức",
    current: "Server updateCheck (camelCase)",
    legacy: "CodePush v0.1 (snake_case)",
    cacheWarning:
      "Kiểm tra cập nhật có thể ảnh hưởng quyết định rollout đã cache ở client. Panel này không ghi nhận trạng thái tải hay triển khai.",
    available: "Có bản cập nhật",
    unavailable: "Không có bản cập nhật",
    noData: "Không có",
    error: "Yêu cầu thất bại",
    requiredEmail: "Nhập địa chỉ email hợp lệ.",
    invalidKey: "Nhập tên và thời hạn là số nguyên từ 1 đến 3650 ngày.",
    transferInvalid: "Xác nhận đúng tên ứng dụng và nhập email của tài khoản khác đã tồn tại.",
    diagnosticsRequired: "Cần deployment key và phiên bản ứng dụng.",
    addSucceeded: "Đã thêm cộng tác viên",
    removed: "Đã gỡ cộng tác viên",
    transferred: "Đã chuyển quyền sở hữu",
    keyRevoked: "Đã thu hồi access key",
    keyCreated: "Đã tạo access key",
    confirmTransfer: (app, email) => `Chuyển ${app} cho ${email}?`,
    confirmRemove: (email) => `Gỡ ${email}?`,
    confirmRevoke: (name) => `Thu hồi ${name}?`,
  },
};

function adminT(key, ...args) {
  const language = typeof currentLang === "string" && currentLang.startsWith("vi") ? "vi" : "en";
  const value = adminText[language][key] || key;
  return typeof value === "function" ? value(...args) : value;
}

function adminEsc(value) {
  return escapeHtml(value == null ? "" : String(value));
}

function adminError(error) {
  toast((error && error.message) || adminT("error"), "error");
}

function adminRows(items, fields, action, actionIndexes, actionLabelKey) {
  const rows = (items || []).slice(0, 100);
  if (!rows.length) return `<p class="text-sm text-onSurface-variant">${adminT("noData")}</p>`;

  const headers = fields.map(([label]) => `<th class="p-2 text-left">${adminEsc(label)}</th>`);
  if (action) {
    const actionLabel = actionLabelKey || (action === "revoke-access-key" ? "revoke" : "remove");
    headers.push(`<th class="p-2 text-left">${adminEsc(adminT(actionLabel))}</th>`);
  }
  const body = rows.map((row, index) => {
    const cells = fields.map(([, key]) => `<td class="p-2 break-all">${adminEsc(row[key])}</td>`);
    if (action && (!actionIndexes || actionIndexes.includes(index))) {
      const labelKey = actionLabelKey || (action === "revoke-access-key" ? "revoke" : "remove");
      const button = `<button class="m3-btn m3-btn-text" type="button" data-admin-action="${action}" data-index="${index}">`;
      cells.push(`<td class="p-2">${button}${adminEsc(adminT(labelKey))}</button></td>`);
    } else if (action) {
      cells.push("<td></td>");
    }
    return `<tr class="border-t border-outline-variant">${cells.join("")}</tr>`;
  });
  const limit = items.length > 100 ? `<p class="mt-2 text-xs">${adminT("listLimit")}</p>` : "";
  const table = `<table class="w-full text-xs"><thead><tr>${headers.join("")}</tr></thead><tbody>${body.join("")}</tbody></table>`;
  return `<div class="overflow-x-auto">${table}</div>${limit}`;
}

function adminFields(fields, formName) {
  const inputs = fields.map((field) => {
    const type = field.type || "text";
    const value = adminEsc(field.value || "");
    const required = field.required ? "required" : "";
    const min = field.min ? `min="${field.min}"` : "";
    const max = field.max ? `max="${field.max}"` : "";
    const step = field.step ? `step="${field.step}"` : "";
    const attributes = `${required} ${min} ${max} ${step}`;
    const input = `<input name="${field.name}" class="m3-field mt-1" type="${type}" value="${value}" ${attributes}>`;
    return `<label class="block text-xs text-onSurface-variant">${adminEsc(field.label)}${input}</label>`;
  });
  return `<form class="space-y-3" data-admin-form="${formName}">${inputs.join("")}</form>`;
}

function adminValue(body, name) {
  return body.querySelector(`[name="${name}"]`)?.value.trim() || "";
}

function adminForm(body, name) {
  return body.querySelector(`[data-admin-form="${name}"]`);
}

async function createAccessKey(body, account) {
  const form = adminForm(body, "access-key");
  if (!form.reportValidity()) return null;

  const friendlyName = adminValue(form, "friendlyName");
  const days = Number(adminValue(form, "ttlDays"));
  if (!friendlyName || !Number.isInteger(days) || days < 1 || days > 3650) {
    toast(adminT("invalidKey"), "error");
    return null;
  }

  const result = await api("POST", "/accessKeys", {
    createdBy: account.email || account.name || "",
    friendlyName,
    ttl: String(days * 86400000),
    description: adminValue(form, "description"),
  });
  return (result.accessKey && result.accessKey.name) || null;
}

function adminAction(label, kind, callback) {
  let busy = false;
  return {
    label,
    kind,
    onClick: async (body) => {
      if (busy) return false;
      busy = true;
      try {
        return await callback(body);
      } finally {
        busy = false;
      }
    },
  };
}

function adminPanel(title, body, actions) {
  return modal({
    title,
    body,
    actions: actions || [{ label: adminT("close"), kind: "secondary", value: null }],
  });
}

async function refreshCurrentApp(appName) {
  await loadApps();
  if (state.currentApp?.name === appName) {
    const refreshed = state.apps.find((app) => app.name === appName);
    if (refreshed) state.currentApp = refreshed;
  }
}

async function openAppSettings() {
  const app = state.currentApp && { ...state.currentApp };
  if (!app) return;

  const isOwner = Object.values(app.collaborators || {}).some(
    (collaborator) => collaborator.isCurrentAccount && collaborator.permission === "Owner",
  );
  try {
    const response = isOwner
      ? await api("GET", `/apps/${encodeURIComponent(app.name)}/collaborators`)
      : {
          collaborators: Object.fromEntries(
            Object.entries(app.collaborators || {}).map(([email, info]) => [
              email,
              { permission: info.permission },
            ]),
          ),
        };
    const entries = Object.entries(response.collaborators || {}).map(([email, info]) => ({
      email,
      permission: info.permission,
    }));
    const removable = entries.flatMap((entry, index) =>
      entry.permission === "Owner" ? [] : [index],
    );
    const body = document.createElement("div");
    body.innerHTML = `${adminRows(
      entries,
      [
        [adminT("email"), "email"],
        [adminT("permission"), "permission"],
      ],
      isOwner && removable.length ? "remove-collaborator" : null,
      removable,
      "remove",
    )}<p class="mt-3 text-xs text-onSurface-variant">${isOwner ? "" : adminT("ownerOnly")}</p>${
      isOwner
        ? `${adminFields([{ name: "email", label: adminT("email"), type: "email", required: true }], "add-collaborator")}${adminFields(
            [
              {
                name: "email",
                label: adminT("email"),
                type: "email",
                required: true,
              },
              { name: "appName", label: adminT("appName"), required: true },
            ],
            "transfer-owner",
          )}`
        : ""
    }`;

    if (isOwner) {
      body.addEventListener("click", async (event) => {
        const button = event.target.closest?.("[data-admin-action]");
        if (!button || button.disabled) return;
        const item = entries[Number(button.dataset.index)];
        if (!item || item.permission === "Owner") return;
        button.disabled = true;
        try {
          if (!(await confirm2(adminT("remove"), adminT("confirmRemove", item.email)))) return;
          await api(
            "DELETE",
            `/apps/${encodeURIComponent(app.name)}/collaborators/${encodeURIComponent(item.email)}`,
          );
          await refreshCurrentApp(app.name);
          button.closest("tr")?.remove();
          toast(adminT("removed"), "success");
        } catch (error) {
          adminError(error);
        } finally {
          if (button.isConnected) button.disabled = false;
        }
      });
    }

    const actions = [{ label: adminT("close"), kind: "secondary", value: null }];
    if (isOwner) {
      actions.push(
        adminAction(adminT("addAction"), "primary", async (bodyEl) => {
          const form = adminForm(bodyEl, "add-collaborator");
          if (!form.reportValidity()) return false;
          const email = adminValue(form, "email");
          if (!email) {
            toast(adminT("requiredEmail"), "error");
            return false;
          }
          await api(
            "POST",
            `/apps/${encodeURIComponent(app.name)}/collaborators/${encodeURIComponent(email)}`,
          );
          await refreshCurrentApp(app.name);
          toast(adminT("addSucceeded"), "success");
          return true;
        }),
      );
      actions.push(
        adminAction(adminT("transferAction"), "danger", async (bodyEl) => {
          const form = adminForm(bodyEl, "transfer-owner");
          if (!form.reportValidity()) return false;
          const name = adminValue(form, "appName");
          const email = adminValue(form, "email");
          const currentEmail = Object.entries(app.collaborators || {}).find(
            ([, info]) => info.isCurrentAccount,
          )?.[0];
          if (
            name !== app.name ||
            !email ||
            (currentEmail && currentEmail.toLowerCase() === email.toLowerCase())
          ) {
            toast(adminT("transferInvalid"), "error");
            return false;
          }
          if (!(await confirm2(adminT("transfer"), adminT("confirmTransfer", app.name, email))))
            return false;

          const originalApp = state.currentApp;
          const originalDeployment = state.currentDeployment;
          await api(
            "POST",
            `/apps/${encodeURIComponent(app.name)}/transfer/${encodeURIComponent(email)}`,
          );
          await loadApps();
          if (
            state.currentApp === originalApp &&
            state.currentDeployment === originalDeployment &&
            state.currentApp?.name === app.name
          ) {
            state.currentApp = null;
            state.currentDeployment = null;
            state.deployments = [];
            state.history = [];
            state.metrics = {};
            clearSelectedReleases();
            renderDeploymentTabs();
            renderHistory();
            if (state.apps.length) await selectApp(state.apps[0].name);
          }
          toast(adminT("transferred"), "success");
          return true;
        }),
      );
    }
    await adminPanel(`${adminT("settings")}: ${app.name}`, body, actions);
  } catch (error) {
    adminError(error);
  }
}

async function openAccessKeys() {
  try {
    const [response, accountResponse] = await Promise.all([
      api("GET", "/accessKeys"),
      api("GET", "/account"),
    ]);
    const keys = response.accessKeys || [];
    const account = accountResponse.account || accountResponse;
    const body = document.createElement("div");
    const expires = (key) => {
      const value = new Date(key.expires).toLocaleString();
      return key.expires <= Date.now() ? `${value} (${adminT("expired")})` : value;
    };
    const rows = keys.map((key) => ({
      key: key.friendlyName,
      createdBy: key.createdBy,
      description: key.description,
      created: new Date(key.createdTime).toLocaleString(),
      expires: expires(key),
    }));
    const rowHeaders = [
      [adminT("keyName"), "key"],
      [adminT("createdBy"), "createdBy"],
      [adminT("description"), "description"],
      [adminT("created"), "created"],
      [adminT("expires"), "expires"],
    ];
    const forms = adminFields(
      [
        { name: "friendlyName", label: adminT("keyName"), required: true },
        {
          name: "ttlDays",
          label: adminT("ttl"),
          type: "number",
          value: "30",
          min: "1",
          max: "3650",
          step: "1",
          required: true,
        },
        { name: "description", label: adminT("description") },
      ],
      "access-key",
    );
    const heading = `<p class="mb-3 text-sm">${adminT("account")}: ${adminEsc(account.email || account.name || "")}</p>`;
    const table = adminRows(
      rows,
      rowHeaders,
      keys.length ? "revoke-access-key" : null,
      undefined,
      "revoke",
    );
    const section = `<h4 class="mt-4 mb-2 font-medium">${adminT("createKey")}</h4>${forms}`;
    body.innerHTML = `${heading}${table}${section}`;

    body.addEventListener("click", async (event) => {
      const button = event.target.closest?.("[data-admin-action]");
      if (!button || button.disabled) return;
      const key = keys[Number(button.dataset.index)];
      if (!key) return;
      button.disabled = true;
      try {
        if (!(await confirm2(adminT("revoke"), adminT("confirmRevoke", key.friendlyName)))) return;
        await api("DELETE", `/accessKeys/${encodeURIComponent(key.friendlyName)}`);
        button.closest("tr")?.remove();
        toast(adminT("keyRevoked"), "success");
      } catch (error) {
        adminError(error);
      } finally {
        if (button.isConnected) button.disabled = false;
      }
    });

    let secret = null;
    const closed = await adminPanel(adminT("keys"), body, [
      { label: adminT("close"), kind: "secondary", value: null },
      adminAction(adminT("createKey"), "primary", async (bodyEl) => {
        secret = await createAccessKey(bodyEl, account);
        return !!secret;
      }),
    ]);
    if (secret) {
      toast(adminT("keyCreated"), "success");
      const secretInput = `<input class="m3-field font-mono" aria-label="${adminEsc(adminT("keys"))}" readonly value="${adminEsc(secret)}">`;
      const secretMessage = `<p class="mb-3 text-sm text-error">${adminT("tokenOnce")}</p>`;
      try {
        await adminPanel(adminT("keys"), `${secretMessage}${secretInput}`, [
          {
            label: adminT("copy"),
            kind: "primary",
            onClick: async () => {
              await navigator.clipboard.writeText(secret);
              toast(adminT("copied"), "success");
              return false;
            },
          },
          { label: adminT("close"), kind: "secondary", value: null },
        ]);
      } finally {
        secret = null;
      }
    } else if (closed) {
      await openAccessKeys();
    }
  } catch (error) {
    adminError(error);
  }
}

async function openStorageAudit() {
  try {
    const report = await api("GET", "/storage/audit");
    const summary = report.summary || {};
    const body = document.createElement("div");
    const summaryEntries = Object.entries(summary).map(
      ([key, value]) => `<dt>${adminEsc(key)}</dt><dd>${adminEsc(value)}</dd>`,
    );
    const summaryList = `<dl class="grid grid-cols-2 gap-2 text-sm">${summaryEntries.join("")}</dl>`;
    const auditNotice = `<p class="mb-3 rounded-lg bg-error-container p-3 text-sm">${adminT("globalWarning")}</p>`;
    const cleanupTitle = adminEsc(adminT("cleanupReason"));
    const disabled = 'type="button" disabled aria-disabled="true"';
    const title = `title="${cleanupTitle}"`;
    const cleanupButton = `<button ${disabled} ${title} class="mt-4 m3-btn m3-btn-outlined opacity-50 cursor-not-allowed">${adminT("cleanup")}</button>`;
    const orphaned = adminRows(report.orphanedFiles || [], [
      [adminT("hash"), "hash"],
      [adminT("size"), "size"],
      [adminT("modified"), "modified"],
      [adminT("path"), "path"],
    ]);
    const missing = adminRows(report.missingFiles || [], [
      [adminT("label"), "label"],
      [adminT("type"), "type"],
      [adminT("hash"), "hash"],
      [adminT("expectedPath"), "expectedPath"],
    ]);
    const valid = adminRows(report.validFiles || [], [
      [adminT("label"), "label"],
      [adminT("type"), "type"],
      [adminT("hash"), "hash"],
      [adminT("size"), "size"],
    ]);
    const orphanedTitle = `<h4 class="mt-4 font-medium">${adminT("orphaned")}</h4>`;
    const missingTitle = `<h4 class="mt-4 font-medium">${adminT("missing")}</h4>`;
    const validTitle = `<h4 class="mt-4 font-medium">${adminT("valid")}</h4>`;
    const cleanupNote = `<p class="mt-2 text-xs text-onSurface-variant">${adminT("cleanupReason")}</p>`;
    body.innerHTML = `${auditNotice}${summaryList}${orphanedTitle}${orphaned}${missingTitle}${missing}${validTitle}${valid}${cleanupButton}${cleanupNote}`;
    await adminPanel(adminT("storage"), body);
  } catch (error) {
    adminError(error);
  }
}

async function openDiagnostics() {
  const deployment = state.currentDeployment && { ...state.currentDeployment };
  const [health, config] = await Promise.allSettled([api("GET", "/health"), api("GET", "/config")]);
  const body = document.createElement("div");
  const healthText = health.status === "fulfilled" ? health.value : health.reason.message;
  const configText =
    config.status === "fulfilled" ? JSON.stringify(config.value, null, 2) : config.reason.message;
  const fields = [
    {
      name: "deploymentKey",
      label: adminT("deploymentKey"),
      value: (deployment && deployment.key) || "",
      required: true,
    },
    { name: "appVersion", label: adminT("appVersion"), required: true },
    { name: "label", label: adminT("label") },
    { name: "packageHash", label: adminT("packageHash") },
    { name: "clientUniqueId", label: adminT("clientUniqueId") },
  ];
  const healthHeading = `<h4 class="font-medium">${adminT("health")}</h4>`;
  const configHeading = `<h4 class="font-medium">${adminT("config")}</h4>`;
  const pre = '<pre class="my-2 whitespace-pre-wrap text-xs">';
  const healthPanel = `${healthHeading}${pre}${adminEsc(healthText)}</pre>`;
  const configPanel = `${configHeading}${pre}${adminEsc(configText)}</pre>`;
  const warning = `<p class="mb-3 text-xs text-error">${adminT("cacheWarning")}</p>`;
  const form = adminFields(fields, "diagnostics");
  const protocolLabel = `<label class="mt-3 block text-xs">${adminT("protocol")}</label>`;
  const protocolOptions = ["current", "legacy"].map(
    (protocol) => `<option value="${protocol}">${adminT(protocol)}</option>`,
  );
  const protocolSelect = `<select name="protocol" class="m3-field">${protocolOptions.join("")}</select>`;
  body.innerHTML = `${healthPanel}${configPanel}${warning}${form}${protocolLabel}${protocolSelect}<div data-result class="mt-3"></div>`;

  await adminPanel(adminT("diagnostics"), body, [
    { label: adminT("close"), kind: "secondary", value: null },
    adminAction(adminT("check"), "primary", async (bodyEl) => {
      const form = adminForm(bodyEl, "diagnostics");
      if (!form.reportValidity()) return false;
      const protocol = bodyEl.querySelector('[name="protocol"]').value;
      const fields =
        protocol === "legacy"
          ? {
              deploymentKey: "deployment_key",
              appVersion: "app_version",
              label: "label",
              packageHash: "package_hash",
              clientUniqueId: "client_unique_id",
            }
          : {
              deploymentKey: "deploymentKey",
              appVersion: "appVersion",
              label: "label",
              packageHash: "packageHash",
              clientUniqueId: "clientUniqueId",
            };
      const params = new URLSearchParams();
      for (const [field, param] of Object.entries(fields)) {
        const value = adminValue(form, field);
        if (value) params.set(param, value);
      }
      const resultEl = bodyEl.querySelector("[data-result]");
      if (!params.get(fields.deploymentKey) || !params.get(fields.appVersion)) {
        toast(adminT("diagnosticsRequired"), "error");
        return false;
      }
      try {
        const path = protocol === "legacy" ? "/v0.1/public/codepush/update_check" : "/updateCheck";
        const response = await api("GET", `${path}?${params}`);
        const update = response.updateInfo || response.update_info || {};
        const available = update.isAvailable ?? update.is_available;
        const status = available ? adminT("available") : adminT("unavailable");
        const responseText = adminEsc(JSON.stringify(response, null, 2));
        resultEl.innerHTML = `<p class="font-medium">${status}</p><pre class="mt-2 whitespace-pre-wrap text-xs">${responseText}</pre>`;
      } catch (error) {
        resultEl.textContent = error.message;
      }
      return false;
    }),
  ]);
}
