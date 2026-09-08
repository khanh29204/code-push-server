// ------------------------ Login page ------------------------
// Depends on: jQuery, auth.js (getAccessToken, logout, parseQuery)

function onLoggedIn() {
  var query = parseQuery();
  if (query.hostname) {
    // came from code-push-cli login
    location.href = '/tokens/' + location.search;
  } else {
    location.href = '/';
  }
}

if (getAccessToken()) {
  onLoggedIn();
}

var submit = false;
$('#submitBtn').on('click', function () {
  if (submit) return;
  submit = true;
  $.ajax({
    type: 'post',
    data: $('#form').serializeArray(),
    url: $('#form').attr('action'),
    dataType: 'json',
    success: function (data) {
      if (data.status == 'OK') {
        localStorage.setItem('auth', data.results.tokens);
        submit = false;
        onLoggedIn();
      } else {
        alert(data.message);
        submit = false;
      }
    },
    error: function () { submit = false; }
  });
});

// Show "Register" button if server allows self-registration
fetch('/config').then(function (r) { return r.json(); }).then(function (config) {
  if (config.allowRegistration) {
    document.getElementById('registerBtn').style.display = 'block';
  }
});

// Lightweight zh-CN i18n (browser language detection)
(function () {
  const dicts = {
    'zh': {
      'Please sign in': '请登录',
      'email address': '邮箱地址',
      'username': '用户名',
      'password': '密码',
      'Log in': '登录',
      'Register': '注册'
    }
  };
  const lang = navigator.language || navigator.userLanguage;
  if (!lang || !lang.startsWith('zh')) return;
  const d = dicts['zh'];
  document.querySelectorAll('[data-i18n]').forEach(function (el) {
    const key = el.getAttribute('data-i18n');
    if (d[key]) el.innerText = d[key];
  });
  document.getElementById('inputEmail').placeholder = d['email address'] + '／' + d['username'];
  document.getElementById('inputPassword').placeholder = d['password'];
})();
