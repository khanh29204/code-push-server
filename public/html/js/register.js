// ------------------------ Register page ------------------------
// Depends on: jQuery, auth.js

authBindForm(async function (form) {
  await authRequest('/users/registerCode', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ email: form.elements.email.value })
  });
  location.href = '/auth/confirm?email=' + encodeURIComponent(form.elements.email.value);
});
