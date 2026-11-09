//! Noticing a sign-in the user typed, so the card can offer to save it, as
//! the Bitwarden extension does. A script in our isolated world, in every
//! frame, watches forms being sent (a submit, a click on a button, Enter in
//! a password field) and hands the address, the username and the password
//! to the app through the binding. Pure.

use serde::Deserialize;

use crate::devtools::BINDING;

pub fn capture_script() -> String {
    format!(
        r#"(function () {{
  if (window.__superfikCapture) return;
  window.__superfikCapture = true;
  var last = '';
  function kind(e) {{ return (e.getAttribute('type') || 'text').toLowerCase(); }}
  function send(root) {{
    var inputs = Array.prototype.slice.call((root || document).querySelectorAll('input'));
    var password = inputs.filter(function (e) {{ return kind(e) === 'password' && e.value; }})[0];
    if (!password) return;
    var username = '';
    for (var i = inputs.indexOf(password) - 1; i >= 0; i--) {{
      var t = kind(inputs[i]);
      if ((t === 'text' || t === 'email' || t === 'tel') && inputs[i].value) {{ username = inputs[i].value; break; }}
    }}
    var message = JSON.stringify({{ url: location.href, username: username, password: password.value }});
    if (message === last) return;
    last = message;
    try {{ {BINDING}(message); }} catch (_) {{}}
  }}
  document.addEventListener('submit', function (event) {{ send(event.target); }}, true);
  document.addEventListener('click', function (event) {{
    var t = event.target, button = t && t.closest && t.closest('button, input[type=submit], [role=button]');
    if (button) send(button.form || button.closest('form'));
  }}, true);
  document.addEventListener('keydown', function (event) {{
    var t = event.target;
    if (event.key === 'Enter' && t && t.tagName === 'INPUT' && kind(t) === 'password') send(t.form);
  }}, true);
}})()"#
    )
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct Submitted {
    pub url: String,
    #[serde(default)]
    pub username: String,
    pub password: Option<String>,
}

impl std::fmt::Debug for Submitted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Submitted")
            .field("url", &self.url)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "…"))
            .finish()
    }
}

impl Submitted {
    pub fn read(payload: &str) -> Option<Self> {
        serde_json::from_str::<Self>(payload).ok().filter(|s| s.password.as_deref().is_some_and(|p| !p.is_empty()))
    }

    pub fn without_password(&self) -> Self {
        Self { password: None, ..self.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_calls_our_binding_once_per_document() {
        let script = capture_script();
        assert!(script.contains(&format!("{BINDING}(message)")));
        assert!(script.contains("__superfikCapture"));
    }

    #[test]
    fn a_sign_in_reads_back_and_never_prints_its_password() {
        let sent =
            Submitted::read(r#"{"url":"https://a.com/login","username":"bruno","password":"hunter2"}"#).expect("read");
        assert_eq!((sent.url.as_str(), sent.username.as_str()), ("https://a.com/login", "bruno"));
        assert!(!format!("{sent:?}").contains("hunter2"));
        assert_eq!(sent.without_password().password, None);
        assert!(Submitted::read(r#"{"url":"https://a.com","username":"b","password":""}"#).is_none());
        assert!(Submitted::read("nonsense").is_none());
    }
}
