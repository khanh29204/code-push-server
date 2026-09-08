// ------------------------ Change password page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, ensureLogin, logout)

ensureLogin();

var submit = false;
$('#submitBtn').on('click', function () {
  if (submit) return;
  submit = true;
  var accessToken = getAccessToken();
  var oldPassword = $('#inputPassword').val();
  var newPassword = $('#inputNewPassword').val();
  $.ajax({
    type: 'patch',
    data: JSON.stringify({ oldPassword: oldPassword, newPassword: newPassword }),
    contentType: 'application/json;charset=utf-8',
    headers: { Authorization: 'Bearer ' + accessToken },
    url: '/users/password',
    dataType: 'json',
    success: function (data) {
      if (data.status == 'OK') {
        alert('change success');
        logout();
      } else if (data.status == 401) {
        alert('token invalid');
        logout();
      } else {
        alert(data.message);
      }
      submit = false;
    },
    error: function () { submit = false; }
  });
});
