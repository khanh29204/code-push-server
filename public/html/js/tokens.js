// ------------------------ Obtain access token page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, ensureLogin, parseQuery)

ensureLogin();

var submit = false;

$('#submitBtn').on('click', function () {
  if (submit) return;
  submit = true;
  var query = parseQuery();
  var createdBy = query.hostname;
  var time = (new Date()).getTime();
  if (createdBy == null || createdBy == undefined || createdBy == '') {
    createdBy = 'Login-' + time;
  }

  // TODO: make ttl and friendlyName configurable
  var postParams = {
    createdBy: createdBy,
    friendlyName: 'Login-' + time,
    ttl: 60 * 60 * 24 * 30 * 1000,
    description: 'Login-' + time,
    isSession: true
  };
  var accessToken = getAccessToken();
  $.ajax({
    type: 'post',
    data: postParams,
    headers: { Authorization: 'Bearer ' + accessToken },
    url: '/accessKeys',
    dataType: 'json',
    success: function (data) {
      submit = false;
      $('#generateSection').hide();
      $('#tipsSuccess').show();
      $('#key').val(data.accessKey.name);
      $('#key').show();
      $('#copyBtn').show();
      $('#tipsClose').show();
    },
    error: function (XMLHttpRequest, textStatus, errorThrown) {
      submit = false;
      if (errorThrown == 'Unauthorized') {
        alert('please login again!');
        location.href = '/auth/login';
      } else {
        alert(errorThrown);
      }
    }
  });
});

$('#copyBtn').on('click', function () {
  var input = document.getElementById('key');
  input.select();
  input.setSelectionRange(0, 99999);
  try {
    navigator.clipboard.writeText(input.value).then(function () {
      $('#copyLabel').text('Copied');
      setTimeout(function () { $('#copyLabel').text('Copy'); }, 1500);
    });
  } catch (e) {
    document.execCommand('copy');
    $('#copyLabel').text('Copied');
    setTimeout(function () { $('#copyLabel').text('Copy'); }, 1500);
  }
});
