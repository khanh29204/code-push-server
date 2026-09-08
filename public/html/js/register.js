// ------------------------ Register page ------------------------
// Depends on: jQuery, auth.js

var submit = false;
$('#submitBtn').on('click', function () {
  if (submit) return;
  submit = true;
  $.ajax({
    type: 'post',
    data: $('#form').serializeArray(),
    url: '/users/registerCode',
    dataType: 'json',
    success: function (data) {
      if (data.status == 'OK') {
        var email = $('#inputEmail').val();
        location.href = '/auth/confirm?email=' + encodeURIComponent(email);
        submit = false;
      } else {
        alert(data.message);
        submit = false;
      }
    },
    error: function () { submit = false; }
  });
});
