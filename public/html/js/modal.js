// ------------------------ Modal ------------------------
let modalSequence = 0;

function modal({ title, body, actions }) {
  return new Promise((resolve) => {
    const root = document.getElementById('modalRoot');
    const returnFocus = document.activeElement;
    const backdrop = document.createElement('div');
    backdrop.className = 'fixed inset-0 z-40 w-screen max-w-[100vw] overflow-y-auto bg-black/40 backdrop-blur-sm flex items-center justify-center p-4';
    const titleId = `modal-title-${++modalSequence}`;
    backdrop.innerHTML = `
      <div role="dialog" aria-modal="true" aria-labelledby="${titleId}" tabindex="-1" class="w-full max-w-lg max-h-[calc(100dvh-2rem)] flex flex-col bg-surface-containerHigh rounded-xl shadow-m3-3 overflow-hidden">
        <div class="px-6 py-4 flex items-center justify-between">
          <h3 id="${titleId}" class="text-xl font-normal text-onSurface">${escapeHtml(title)}</h3>
          <button data-close aria-label="${escapeHtml(t('close'))}" class="m3-icon-btn h-9 w-9"><span class="material-symbols-rounded" style="font-size:20px">close</span></button>
        </div>
        <div class="px-6 py-2 flex-1 min-h-0 overflow-y-auto scrollbar-thin" data-body></div>
        <div class="px-6 py-4 flex flex-wrap justify-end gap-2" data-actions></div>
      </div>`;
    const dialog = backdrop.querySelector('[role="dialog"]');
    const bodyEl = backdrop.querySelector('[data-body]');
    const actionsEl = backdrop.querySelector('[data-actions]');
    if (typeof body === 'string') bodyEl.innerHTML = body;
    else bodyEl.appendChild(body);

    let pending = false;
    const initiallyDisabled = new Map([...backdrop.querySelectorAll('button')].map(button => [button, button.disabled]));
    bodyEl.addEventListener('submit', e => e.preventDefault());
    function setPending(value) {
      pending = value;
      backdrop.querySelectorAll('button').forEach(button => {
        button.disabled = value || (initiallyDisabled.get(button) ?? false);
      });
    }
    function close(val) {
      if (pending || backdrop.parentElement?.lastElementChild !== backdrop) return;
      document.removeEventListener('keydown', onKeyDown, true);
      backdrop.remove();
      if (returnFocus?.isConnected) returnFocus.focus();
      resolve(val);
    }
    function onKeyDown(e) {
      if (backdrop.parentElement?.lastElementChild !== backdrop) return;
      if (pending) { if (e.key === 'Escape') e.preventDefault(); return; }
      if (e.key === 'Escape') { e.stopImmediatePropagation(); e.preventDefault(); close(null); return; }
      if (e.key !== 'Tab') return;
      e.stopImmediatePropagation();
      const focusable = [...dialog.querySelectorAll('button:not([disabled]),input:not([disabled]),textarea:not([disabled]),select:not([disabled]),[tabindex]:not([tabindex="-1"])')];
      if (!focusable.length) { e.preventDefault(); dialog.focus(); return; }
      if (e.shiftKey && document.activeElement === focusable[0]) { e.preventDefault(); focusable[focusable.length - 1].focus(); }
      else if (!e.shiftKey && document.activeElement === focusable[focusable.length - 1]) { e.preventDefault(); focusable[0].focus(); }
    }
    backdrop.querySelector('[data-close]').onclick = () => close(null);
    backdrop.addEventListener('click', e => { if (e.target === backdrop && !pending) close(null); });
    document.addEventListener('keydown', onKeyDown, true);

    (actions || [{ label: t('close'), kind: 'secondary', value: null }]).forEach(a => {
      const btn = document.createElement('button');
      if (a.id) btn.id = a.id;
      const base = 'm3-btn';
      const styles = { primary: 'm3-btn-filled', secondary: 'm3-btn-text', danger: 'm3-btn-filled' };
      btn.className = base + ' ' + (styles[a.kind] || styles.secondary);
      if (a.kind === 'danger') btn.style.backgroundColor = '#B3261E';
      btn.textContent = a.label;
      btn.onclick = async () => {
        if (a.onClick) {
          setPending(true);
          try {
            const r = await a.onClick(bodyEl);
            if (r !== false) { pending = false; close(a.value !== undefined ? a.value : true); }
          } catch (e) { toast(e.message, 'error'); }
          finally { if (btn.isConnected && backdrop.isConnected) setPending(false); }
        } else close(a.value !== undefined ? a.value : true);
      };
      actionsEl.appendChild(btn);
    });

    root.appendChild(backdrop);
    const firstInput = bodyEl.querySelector('input,textarea,select');
    (firstInput || dialog).focus();
  });
}

function prompt2(title, fields, submitLabel = 'Save') {
  return new Promise((resolve) => {
    let resolved = false;
    const formId = `modal-form-${++modalSequence}`;
    const form = document.createElement('form');
    form.className = 'space-y-3';
    fields.forEach(f => {
      const wrap = document.createElement('div');
      const fieldId = `${formId}-${f.name}`;
      const label = `<label for="${fieldId}" class="block text-xs font-medium text-onSurface-variant mb-1">${escapeHtml(f.label)}</label>`;
      let input = '';
      if (f.type === 'textarea') {
        input = `<textarea id="${fieldId}" name="${f.name}" rows="3" class="m3-field">${escapeHtml(f.value ?? '')}</textarea>`;
      } else if (f.type === 'checkbox') {
        input = `<label for="${fieldId}" class="inline-flex items-center gap-2 text-sm text-onSurface"><input id="${fieldId}" type="checkbox" name="${f.name}" ${f.value ? 'checked' : ''} class="rounded border-outline text-primary focus:ring-primary/30 accent-primary"> ${escapeHtml(f.checkboxLabel || f.label || '')}</label>`;
      } else if (f.type === 'select') {
        input = `<select id="${fieldId}" name="${f.name}" class="m3-field">${
          (f.options || []).map(o => `<option value="${escapeHtml(o.value)}" ${o.value == f.value ? 'selected' : ''}>${escapeHtml(o.label)}</option>`).join('')
        }</select>`;
      } else if (f.type === 'number' || f.type === 'range') {
        input = `<input id="${fieldId}" type="${f.type}" name="${f.name}" value="${escapeHtml(f.value ?? '')}" min="${f.min ?? ''}" max="${f.max ?? ''}" step="${f.step ?? ''}" ${f.required ? 'required' : ''} class="m3-field">`;
      } else {
        input = `<input id="${fieldId}" type="text" name="${f.name}" value="${escapeHtml(f.value ?? '')}" placeholder="${escapeHtml(f.placeholder || '')}" ${f.required ? 'required' : ''} class="m3-field">`;
      }
      wrap.innerHTML = f.type === 'checkbox' ? input : label + input + (f.hint ? `<p class="mt-1 text-xs text-onSurface-variant">${escapeHtml(f.hint)}</p>` : '');
      form.appendChild(wrap);
    });

    let result = null;
    function submit() {
      if (resolved || !form.reportValidity()) return false;
      const out = {};
      fields.forEach(f => {
        const el = form.elements[f.name];
        if (!el) return;
        if (f.type === 'checkbox') out[f.name] = el.checked;
        else if (f.type === 'number' || f.type === 'range') out[f.name] = el.value === '' ? null : Number(el.value);
        else out[f.name] = el.value;
      });
      result = out;
      return true;
    }
    form.addEventListener('submit', e => {
      e.preventDefault();
      document.getElementById(`${formId}-submit`)?.click();
    });
    form.addEventListener('keydown', e => {
      if (e.key !== 'Enter' || e.target.tagName !== 'INPUT') return;
      e.preventDefault();
      document.getElementById(`${formId}-submit`)?.click();
    });
    modal({
      title,
      body: form,
      actions: [
        { label: t('cancel'), kind: 'secondary', value: null },
        { id: `${formId}-submit`, label: submitLabel, kind: 'primary', onClick: submit },
      ]
    }).then(v => {
      resolved = true;
      resolve(v === null ? null : result);
    });
  });
}

function confirm2(title, msg, danger = true) {
  return modal({
    title,
    body: `<p class="text-sm text-onSurface-variant">${escapeHtml(msg)}</p>`,
    actions: [
      { label: t('cancel'), kind: 'secondary', value: false },
      { label: t('confirm'), kind: danger ? 'danger' : 'primary', value: true },
    ]
  });
}
