//! `{{…}}` placeholders in manifest strings: `{{secret.<key>}}`,
//! `{{account}}` (the account's label) and `{{server}}` (the MCP server
//! name the run gives this account). Pure.

use std::collections::BTreeMap;

use crate::error::PluginError;

#[derive(Debug, Clone, Copy)]
pub struct Vars<'a> {
    pub secrets: &'a BTreeMap<String, String>,
    pub account: &'a str,
    pub server: &'a str,
}

pub fn references(text: &str) -> Result<Vec<&str>, PluginError> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| PluginError::Template(format!("unclosed `{{{{` in `{text}`")))?;
        found.push(after[..end].trim());
        rest = &after[end + 2..];
    }
    Ok(found)
}

pub fn render(text: &str, vars: Vars) -> Result<String, PluginError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| PluginError::Template(format!("unclosed `{{{{` in `{text}`")))?;
        out.push_str(value(after[..end].trim(), vars)?);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

fn value<'a>(name: &str, vars: Vars<'a>) -> Result<&'a str, PluginError> {
    match name {
        "account" => Ok(vars.account),
        "server" => Ok(vars.server),
        _ => name
            .strip_prefix("secret.")
            .and_then(|key| vars.secrets.get(key))
            .map(String::as_str)
            .ok_or_else(|| PluginError::Template(format!("no value for `{name}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_secrets_account_and_server() {
        let secrets = BTreeMap::from([("api_key".to_owned(), "k-1".to_owned())]);
        let vars = Vars { secrets: &secrets, account: "work", server: "gmail-work" };
        assert_eq!(render("Bearer {{ secret.api_key }}", vars), Ok("Bearer k-1".to_owned()));
        assert_eq!(render("{{account}} via {{server}}!", vars), Ok("work via gmail-work!".to_owned()));
        assert_eq!(render("plain", vars), Ok("plain".to_owned()));
    }

    #[test]
    fn unknown_and_unclosed_placeholders_fail() {
        let secrets = BTreeMap::new();
        let vars = Vars { secrets: &secrets, account: "", server: "" };
        assert!(render("{{secret.token}}", vars).is_err());
        assert!(render("{{account", vars).is_err());
        assert_eq!(references("a {{x}} b {{ secret.y }}"), Ok(vec!["x", "secret.y"]));
    }
}
