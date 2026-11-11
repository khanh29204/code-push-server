// ------------------------ Change password page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, ensureLogin, logout)

var pageReady = ensureLogin();

authBindForm(async function (form) {
  if (!(await pageReady)) return;
  var oldPassword = form.elements.oldPassword.value;
  var newPassword = form.elements.newPassword.value;
  if (newPassword.length < 6 || newPassword.length > 20) {
    throw new Error('Please enter a new password between 6 and 20 characters long.');
  }
  await authRequest('/users/password', {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + getAccessToken() },
    body: JSON.stringify({ oldPassword: oldPassword, newPassword: newPassword })
  });
  logout();
});
