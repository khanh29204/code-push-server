// ------------------------ Obtain access token page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, ensureLogin, parseQuery)

var pageReady = ensureLogin();
var generating = false;

async function generateToken(button) {
  if (generating) return;
  generating = true;
  button.disabled = true;
  try {
    if (!(await pageReady)) return;
    authShowError('');
    var query = parseQuery();
    var time = (new Date()).getTime();
    var createdBy = query.hostname || 'Login-' + time;
    var postParams = {
      createdBy: createdBy,
      friendlyName: 'Login-' + time,
      ttl: String(60 * 60 * 24 * 30 * 1000),
      description: 'Login-' + time
    };
    var data = await authRequest('/accessKeys', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + getAccessToken() },
      body: JSON.stringify(postParams)
    });
    if (!data.accessKey || !data.accessKey.name) throw new Error('The server did not return an access token.');
    $('#generateSection').hide();
    $('#tipsSuccess').show();
    $('#key').val(data.accessKey.name).show();
    $('#copyBtn').show();
    $('#tipsClose').show();
  } catch (error) {
    authShowError(error.message);
  } finally {
    generating = false;
    button.disabled = false;
  }
}

$('#submitBtn').on('click', function () { return generateToken(this); });

$('#copyBtn').on('click', async function () {
  var input = document.getElementById('key');
  input.select();
  input.setSelectionRange(0, 99999);
  try {
    await navigator.clipboard.writeText(input.value);
  } catch (error) {
    if (!document.execCommand('copy')) {
      authShowError('Copy failed. Select and copy the token manually.');
      return;
    }
  }
  $('#copyLabel').text('Copied');
  setTimeout(function () { $('#copyLabel').text('Copy'); }, 1500);
});
