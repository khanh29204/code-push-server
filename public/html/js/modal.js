// ------------------------ Modal ------------------------

function modal({ title, body, actions }) {
  return new Promise((resolve) => {
    const root = document.getElementById('modalRoot');
    const backdrop = document.createElement('div');
    backdrop.className = 'fixed inset-0 z-40 bg-black/40 backdrop-blur-sm flex items-center justify-center p-4';
    backdrop.innerHTML = `
      <div class="w-full max-w-lg bg-surface-containerHigh rounded-xl shadow-m3-3 overflow-hidden">
        <div class="px-6 py-4 flex items-center justify-between">
          <h3 class="text-xl font-normal text-onSurface">${escapeHtml(title)}</h3>
          <button data-close class="m3-icon-btn h-9 w-9"><span class="material-symbols-rounded" style="font-size:20px">close</span></button>
        </div>
        <div class="px-6 py-2 max-h-[70vh] overflow-y-auto scrollbar-thin" data-body></div>
        <div class="px-6 py-4 flex justify-end gap-2" data-actions></div>
      </div>`;
    const bodyEl = backdrop.querySelector('[data-body]');
    const actionsEl = backdrop.querySelector('[data-actions]');
    if (typeof body === 'string') bodyEl.innerHTML = body;
    else bodyEl.appendChild(body);

    function close(val) { backdrop.remove(); resolve(val); }
    backdrop.querySelector('[data-close]').onclick = () => close(null);
    backdrop.addEventListener('click', e => { if (e.target === backdrop) close(null); });

    (actions || [{ label: 'Close', kind: 'secondary', value: null }]).forEach(a => {
      const btn = document.createElement('button');
      const base = 'm3-btn';
      const styles = {
        primary: 'm3-btn-filled',
        secondary: 'm3-btn-text',
        danger: 'm3-btn-filled',
      };
      btn.className = base + ' ' + (styles[a.kind] || styles.secondary);
      if (a.kind === 'danger') btn.style.backgroundColor = '#B3261E';
      btn.textContent = a.label;
      btn.onclick = async () => {
        if (a.onClick) {
          try {
            const r = await a.onClick(bodyEl);
            if (r !== false) close(a.value !== undefined ? a.value : true);
          } catch (e) { toast(e.message, 'error'); }
        } else close(a.value !== undefined ? a.value : true);
      };
      actionsEl.appendChild(btn);
    });

    root.appendChild(backdrop);
    const firstInput = bodyEl.querySelector('input,textarea,select');
    if (firstInput) firstInput.focus();
  });
}

function prompt2(title, fields, submitLabel = 'Save') {
  return new Promise((resolve) => {
    const form = document.createElement('form');
    form.className = 'space-y-3';
    fields.forEach(f => {
      const wrap = document.createElement('div');
      const label = `<label class="block text-xs font-medium text-onSurface-variant mb-1">${escapeHtml(f.label)}</label>`;
      let input = '';
      if (f.type === 'textarea') {
        input = `<textarea name="${f.name}" rows="3" class="m3-field">${escapeHtml(f.value ?? '')}</textarea>`;
      } else if (f.type === 'checkbox') {
        input = `<label class="inline-flex items-center gap-2 text-sm text-onSurface"><input type="checkbox" name="${f.name}" ${f.value ? 'checked' : ''} class="rounded border-outline text-primary focus:ring-primary/30 accent-primary"> ${escapeHtml(f.checkboxLabel || '')}</label>`;
      } else if (f.type === 'select') {
        input = `<select name="${f.name}" class="m3-field">${
          (f.options || []).map(o => `<option value="${escapeHtml(o.value)}" ${o.value == f.value ? 'selected' : ''}>${escapeHtml(o.label)}</option>`).join('')
        }</select>`;
      } else if (f.type === 'number' || f.type === 'range') {
        input = `<input type="${f.type}" name="${f.name}" value="${escapeHtml(f.value ?? '')}" min="${f.min ?? ''}" max="${f.max ?? ''}" step="${f.step ?? ''}" class="m3-field">`;
      } else {
        input = `<input type="text" name="${f.name}" value="${escapeHtml(f.value ?? '')}" placeholder="${escapeHtml(f.placeholder || '')}" class="m3-field">`;
      }
      wrap.innerHTML = f.type === 'checkbox' ? input : label + input + (f.hint ? `<p class="mt-1 text-xs text-onSurface-variant">${escapeHtml(f.hint)}</p>` : '');
      form.appendChild(wrap);
    });

    modal({
      title,
      body: form,
      actions: [
        { label: 'Cancel', kind: 'secondary', value: null },
        {
          label: submitLabel, kind: 'primary', onClick: () => {
            const out = {};
            fields.forEach(f => {
              const el = form.elements[f.name];
              if (!el) return;
              if (f.type === 'checkbox') out[f.name] = el.checked;
              else if (f.type === 'number' || f.type === 'range') out[f.name] = el.value === '' ? null : Number(el.value);
              else out[f.name] = el.value;
            });
            resolve(out);
          }
        }
      ]
    }).then(v => { if (v === null) resolve(null); });
  });
}

function confirm2(title, msg, danger = true) {
  return modal({
    title,
    body: `<p class="text-sm text-onSurface-variant">${escapeHtml(msg)}</p>`,
    actions: [
      { label: 'Cancel', kind: 'secondary', value: false },
      { label: 'Confirm', kind: danger ? 'danger' : 'primary', value: true },
    ]
  });
}
