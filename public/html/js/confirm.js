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

var submit = false;
$('#submitBtn').on('click', function () {
  if (submit) return;
  console.log($('#form').serializeArray());
  submit = true;
  $.ajax({
    type: 'post',
    data: $('#form').serializeArray(),
    url: '/users/',
    dataType: 'json',
    success: function (data) {
      if (data.status == 'OK') {
        showModal();
        submit = false;
      } else {
        alert(data.message);
        submit = false;
      }
    },
    error: function () { submit = false; }
  });
});
