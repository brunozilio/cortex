//! The two scripts a login fill runs in the page, as the Bitwarden
//! extension does: one reads the form fields (no values, no secrets), the
//! other places a login into the fields `superfik_core::autofill::plan`
//! chose, as a user typing would, once the page is still the one it was
//! chosen for. Pure.

use serde::Deserialize;
use superfik_core::autofill::{FillOutcome, FillPlan, LoginFill, PageField};

const TAG: &str = "data-superfik-fill";

pub fn read_script() -> String {
    format!(
        r#"(function () {{
  var TAG = '{TAG}', forms = Array.prototype.slice.call(document.forms), n = 0, fields = [];
  document.querySelectorAll('[' + TAG + ']').forEach(function (e) {{ e.removeAttribute(TAG); }});
  var TYPES = {{ '': 1, text: 1, email: 1, tel: 1, password: 1, search: 1, number: 1 }};
  function label(e) {{
    var l = (e.labels && e.labels[0]) || e.closest('label');
    var t = e.getAttribute('aria-label') || (l ? l.innerText : '');
    return (t || '').replace(/\s+/g, ' ').trim().slice(0, 100);
  }}
  function visible(e) {{
    if (e.getClientRects().length === 0) return false;
    var s = getComputedStyle(e), r = e.getBoundingClientRect();
    return s.visibility !== 'hidden' && s.opacity !== '0' && r.width > 1 && r.height > 1;
  }}
  document.querySelectorAll('input').forEach(function (e) {{
    var type = (e.getAttribute('type') || '').toLowerCase();
    if (!TYPES[type]) return;
    n += 1;
    e.setAttribute(TAG, String(n));
    var form = e.form ? forms.indexOf(e.form) : -1;
    fields.push({{
      ref: n, type: type, name: e.name || '', id: e.id || '',
      autocomplete: e.getAttribute('autocomplete') || '', placeholder: e.placeholder || '',
      label: label(e), form: form < 0 ? null : form, visible: visible(e),
      readonly: e.readOnly, disabled: e.disabled, focused: document.activeElement === e
    }});
  }});
  return {{ url: location.href, fields: fields }};
}})()"#
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReadPage {
    pub url: String,
    pub fields: Vec<PageField>,
}

pub fn read_page(json: &str) -> Result<ReadPage, String> {
    serde_json::from_str(json).map_err(|error| format!("could not read the page's fields: {error}"))
}

pub fn fill_script(plan: FillPlan, fill: &LoginFill, origin: &str, path: &str) -> String {
    let pairs: Vec<(u32, &str)> =
        [(plan.username, fill.username.as_deref()), (plan.password, fill.password.as_deref())]
            .into_iter()
            .filter_map(|(at, value)| Some((at?, value?)))
            .collect();
    let values: Vec<String> = pairs.iter().map(|(at, value)| format!("[{at}, {}]", quoted(value))).collect::<Vec<_>>();
    format!(
        r#"(function () {{
  if (location.origin !== {origin} || location.pathname !== {path}) return {{ moved: true }};
  var setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set, filled = 0;
  function fire(e, type) {{ e.dispatchEvent(new Event(type, {{ bubbles: true }})); }}
  function key(e, type) {{ e.dispatchEvent(new KeyboardEvent(type, {{ bubbles: true }})); }}
  [{values}].forEach(function (pair) {{
    var e = document.querySelector('[{TAG}="' + pair[0] + '"]');
    if (!e || e.disabled || e.readOnly) return;
    e.click(); e.focus();
    key(e, 'keydown'); key(e, 'keyup');
    setter.call(e, pair[1]);
    key(e, 'keydown'); key(e, 'keyup');
    fire(e, 'input'); fire(e, 'change');
    filled += 1;
  }});
  return {{ filled: filled }};
}})()"#,
        origin = quoted(origin),
        path = quoted(path),
        values = values.join(", "),
    )
}

pub fn fill_outcome(json: &str) -> FillOutcome {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default)]
        moved: bool,
        #[serde(default)]
        filled: u32,
    }
    match serde_json::from_str::<Answer>(json) {
        Ok(Answer { moved: true, .. }) | Err(_) => FillOutcome::Denied,
        Ok(Answer { filled: 0, .. }) => FillOutcome::Absent,
        Ok(_) => FillOutcome::Filled,
    }
}

fn quoted(text: &str) -> String {
    serde_json::to_string(text)
        .unwrap_or_else(|_| "\"\"".to_owned())
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill() -> LoginFill {
        LoginFill { username: Some("bruno".into()), password: Some("p\"');alert(1)//\u{2028}".into()) }
    }

    #[test]
    fn the_read_tags_fields_and_reads_back() {
        assert!(read_script().contains(TAG));
        let page = read_page(r#"{"url":"https://a.com/login","fields":[{"ref":1,"type":"password","form":null,"visible":true,"readonly":false,"disabled":false,"focused":true,"name":"","id":"","autocomplete":"","placeholder":"","label":""}]}"#)
            .expect("reads");
        assert_eq!(page.url, "https://a.com/login");
        assert!(page.fields[0].focused);
        assert!(read_page("null").is_err());
    }

    #[test]
    fn the_fill_quotes_values_and_checks_the_page_first() {
        let plan = FillPlan { username: Some(2), password: Some(3) };
        let script = fill_script(plan, &fill(), "https://a.com", "/login");
        assert!(script.contains(r#"location.origin !== "https://a.com" || location.pathname !== "/login""#));
        assert!(script.contains(r#"[2, "bruno"], [3, "p\"');alert(1)//\u2028"]"#), "{script}");
    }

    #[test]
    fn the_fill_skips_what_the_plan_or_the_login_lacks() {
        let only_username = LoginFill { username: Some("bruno".into()), password: None };
        let script = fill_script(FillPlan { username: Some(1), password: Some(2) }, &only_username, "o", "/");
        assert!(script.contains(r#"[[1, "bruno"]]"#));
        let script = fill_script(FillPlan { username: None, password: Some(2) }, &fill(), "o", "/");
        assert!(!script.contains("bruno"));
    }

    #[test]
    fn outcomes_read_back() {
        assert_eq!(fill_outcome(r#"{"filled":2}"#), FillOutcome::Filled);
        assert_eq!(fill_outcome(r#"{"filled":0}"#), FillOutcome::Absent);
        assert_eq!(fill_outcome(r#"{"moved":true}"#), FillOutcome::Denied);
        assert_eq!(fill_outcome("oops"), FillOutcome::Denied);
    }
}
