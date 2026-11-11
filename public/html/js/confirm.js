// ------------------------ Confirm page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, parseQuery)

// Lightweight modal helpers (replaces Bootstrap JS)
function showModal() { $('#myModal').removeClass('hidden').addClass('flex'); }
function hideModal() { $('#myModal').removeClass('flex').addClass('hidden'); }
$('[data-dismiss="modal"]').on('click', hideModal);

// Prefill email from query string if present
(function () {
  var q = parseQuery();
  if (q.email) { $('#inputEmail').val(q.email); }
})();

$('#okBtn').on('click', function () {
  location.href = '/auth/login?email=' + encodeURIComponent($('#inputEmail').val());
});

authBindForm(async function (form) {
  await authRequest('/users/', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email: form.elements.email.value,
      token: form.elements.token.value,
      password: form.elements.password.value
    })
  });
  showModal();
});
