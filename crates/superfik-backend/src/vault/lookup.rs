//! Finding the secrets a call names among the run's folders, and listing
//! them for the agent without their values. Pure.

use superfik_core::SecretUse;

use super::provider::Item;

#[derive(Debug, Clone)]
pub struct Folder {
    pub title: String,
    pub name: String,
    pub items: Vec<Item>,
}

pub fn listing(folders: &[Folder]) -> String {
    let mut out = Vec::new();
    for folder in folders {
        out.push(format!("{} (folder {}):", folder.title, folder.name));
        if folder.items.is_empty() {
            out.push("- (no items)".into());
        }
        for item in &folder.items {
            let fields: Vec<&str> = item.values.keys().map(String::as_str).collect();
            out.push(format!("- {}: {}", item.name, fields.join(", ")));
        }
    }
    out.join("\n")
}

pub fn resolve(folders: &[Folder], uses: &[SecretUse]) -> Result<Vec<(String, String)>, String> {
    uses.iter().map(|u| Ok((u.env.clone(), value(find(folders, &u.item)?, &u.field)?.to_owned()))).collect()
}

fn find<'a>(folders: &'a [Folder], wanted: &str) -> Result<&'a Item, String> {
    let mut found: Vec<&Item> = folders.iter().flat_map(|f| named(f, wanted)).collect();
    if found.is_empty()
        && let Some((folder, name)) = wanted.split_once('/')
    {
        found = folders
            .iter()
            .filter(|f| f.name.eq_ignore_ascii_case(folder.trim()))
            .flat_map(|f| named(f, name.trim()))
            .collect();
    }
    match found.as_slice() {
        [item] => Ok(item),
        [] => Err(format!("No item is named `{wanted}`; list_secrets shows the ones you may use.")),
        _ => Err(format!("Several items are named `{wanted}`; name one as Folder/Item.")),
    }
}

fn named<'a>(folder: &'a Folder, name: &str) -> Vec<&'a Item> {
    folder.items.iter().filter(|item| item.name.eq_ignore_ascii_case(name)).collect()
}

fn value<'a>(item: &'a Item, field: &str) -> Result<&'a str, String> {
    let found = item
        .values
        .get(field)
        .or_else(|| item.values.iter().find(|(k, _)| k.eq_ignore_ascii_case(field)).map(|(_, v)| v));
    found.map(String::as_str).ok_or_else(|| {
        let fields: Vec<&str> = item.values.keys().map(String::as_str).collect();
        format!("`{}` has no `{field}` field (it has: {}).", item.name, fields.join(", "))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn item(name: &str, fields: &[(&str, &str)]) -> Item {
        Item {
            name: name.into(),
            values: fields.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect::<BTreeMap<_, _>>(),
            uris: Vec::new(),
        }
    }

    fn folders() -> Vec<Folder> {
        vec![
            Folder {
                title: "Bitwarden · Work".into(),
                name: "Work".into(),
                items: vec![
                    item("GitHub", &[("password", "w-pass"), ("Token", "w-tok")]),
                    item("npm", &[("password", "n")]),
                ],
            },
            Folder {
                title: "Bitwarden · Home".into(),
                name: "Home".into(),
                items: vec![item("GitHub", &[("password", "h-pass")])],
            },
            Folder { title: "Bitwarden · Empty".into(), name: "Empty".into(), items: vec![] },
        ]
    }

    fn uses(item: &str, field: &str) -> Vec<SecretUse> {
        vec![SecretUse { item: item.into(), field: field.into(), env: "T".into() }]
    }

    #[test]
    fn listings_show_names_and_fields_never_values() {
        let text = listing(&folders());
        assert_eq!(
            text,
            "Bitwarden · Work (folder Work):\n- GitHub: Token, password\n- npm: password\n\
             Bitwarden · Home (folder Home):\n- GitHub: password\n\
             Bitwarden · Empty (folder Empty):\n- (no items)"
        );
        assert!(!text.contains("w-pass"));
    }

    #[test]
    fn secrets_resolve_by_name_folder_and_field() {
        assert_eq!(resolve(&folders(), &uses("NPM", "password")), Ok(vec![("T".into(), "n".into())]));
        assert_eq!(resolve(&folders(), &uses("home/github", "password")), Ok(vec![("T".into(), "h-pass".into())]));
        assert_eq!(resolve(&folders(), &uses("Work/GitHub", "token")), Ok(vec![("T".into(), "w-tok".into())]));
        assert!(resolve(&folders(), &uses("GitHub", "password")).is_err_and(|e| e.contains("Folder/Item")));
        assert!(resolve(&folders(), &uses("Slack", "password")).is_err_and(|e| e.contains("list_secrets")));
        assert!(resolve(&folders(), &uses("npm", "otp")).is_err_and(|e| e.contains("it has: password")));
    }
}
