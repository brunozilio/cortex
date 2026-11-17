use std::path::{Path, PathBuf};

use serde_json::Value;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];

const CACHES: [&str; 13] = [
    "Library/Caches",
    ".cache",
    ".npm",
    ".cargo/registry",
    ".cargo/git",
    ".cargo/.package-cache",
    ".cargo/.package-cache-mutate",
    ".cargo/.global-cache",
    "go/pkg",
    ".gradle",
    ".rustup",
    ".pnpm-store",
    ".yarn",
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sandbox {
    pub allow_write: Vec<String>,
    pub network: bool,
    #[serde(default = "required_by_default")]
    pub required: bool,
}

pub const REQUIRED_BY_DEFAULT: bool = cfg!(any(target_os = "linux", target_os = "macos"));

fn required_by_default() -> bool {
    REQUIRED_BY_DEFAULT
}

impl Default for Sandbox {
    fn default() -> Self {
        Self { allow_write: Vec::new(), network: true, required: REQUIRED_BY_DEFAULT }
    }
}

fn turning_it_off() -> String {
    format!(r#"set "sandbox": {{ "required": false }} in ~/{}/settings.json"#, brand::DIR)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub notice: String,
    pub refuses: bool,
}

pub fn refusal(why: &str) -> String {
    format!(
        "Refused: the sandbox is required (sandbox.required) but {why}, so commands cannot be confined. Tell the user; \
         they can run it themselves, or {} to let commands run unconfined.",
        turning_it_off()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places<'a> {
    pub project: &'a Path,
    pub added: &'a [PathBuf],
    pub repository: Option<&'a Path>,
    pub temp: &'a [PathBuf],
    pub home: Option<&'a Path>,
    pub settings_dir: &'a str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Confinement {
    pub writable: Vec<PathBuf>,
    pub protected: Vec<PathBuf>,
    pub network: bool,
}

impl Sandbox {
    pub fn missing(&self, unavailable: Option<&str>) -> Option<Missing> {
        let why = unavailable?;
        let notice = if self.required {
            format!(
                "The sandbox is required (sandbox.required) but {why}: shell commands the agent runs are refused. To \
                 run them unconfined, {}.",
                turning_it_off()
            )
        } else {
            format!(
                "The sandbox is on but {why}: shell commands run unconfined. Set sandbox.required to true to refuse \
                 them instead."
            )
        };
        Some(Missing { notice, refuses: self.required })
    }

    pub fn layered(mut self, value: &Value, owners: bool) -> Result<Self, String> {
        if let Some(flag) = value.get("network") {
            let flag = flag.as_bool().ok_or("sandbox.network must be true or false")?;
            self.network = if owners { flag } else { self.network && flag };
        }
        if let Some(flag) = value.get("required") {
            let flag = flag.as_bool().ok_or("sandbox.required must be true or false")?;
            self.required = if owners { flag } else { self.required || flag };
        }
        if let Some(paths) = value.get("allowWrite") {
            let paths = paths.as_array().ok_or("sandbox.allowWrite must be a list of folders")?;
            let named = paths.iter().map(|path| path.as_str().map(str::to_owned));
            let named: Option<Vec<String>> = named.collect();
            let named = named.ok_or("sandbox.allowWrite must be a list of folders")?;
            if owners {
                self.allow_write.extend(named);
            }
        }
        Ok(self)
    }

    pub fn confinement(&self, places: &Places<'_>) -> Confinement {
        let mut writable = vec![places.project.to_path_buf()];
        writable.extend(places.added.iter().cloned());
        writable.extend(places.repository.map(Path::to_path_buf));
        writable.extend(places.temp.iter().cloned());
        if let Some(home) = places.home {
            writable.extend(CACHES.iter().map(|cache| home.join(cache)));
        }
        writable.extend(self.allow_write.iter().filter_map(|path| owned(path, places.home)));
        let mut kept = Vec::new();
        for path in writable {
            if !kept.contains(&path) {
                kept.push(path);
            }
        }
        let roots = std::iter::once(places.project).chain(places.added.iter().map(PathBuf::as_path));
        let protected =
            roots.flat_map(|root| SETTINGS.iter().map(move |name| root.join(places.settings_dir).join(name))).collect();
        Confinement { writable: kept, protected, network: self.network }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ServerSandbox {
    pub off: bool,
    pub writable: Vec<String>,
    pub network: Option<bool>,
}

impl ServerSandbox {
    pub fn read(value: &Value) -> Result<Self, String> {
        match value {
            Value::Null | Value::Bool(true) => Ok(Self::default()),
            Value::Bool(false) => Ok(Self { off: true, ..Self::default() }),
            Value::Object(fields) => {
                let writable = match fields.get("writable") {
                    None => Vec::new(),
                    Some(listed) => {
                        let listed = listed.as_array().ok_or("sandbox.writable must be a list of folders")?;
                        let named: Option<Vec<String>> =
                            listed.iter().map(|path| path.as_str().map(str::to_owned)).collect();
                        named.ok_or("sandbox.writable must be a list of folders")?
                    }
                };
                let network = match fields.get("network") {
                    None => None,
                    Some(flag) => Some(flag.as_bool().ok_or("sandbox.network must be true or false")?),
                };
                Ok(Self { off: false, writable, network })
            }
            _ => Err("sandbox must be false or an object { writable, network }".to_owned()),
        }
    }

    pub fn as_declared_by(self, owner: bool) -> Self {
        if owner {
            return self;
        }
        Self { off: false, writable: Vec::new(), network: self.network.filter(|network| !network) }
    }
}

impl Sandbox {
    pub fn server_confinement(&self, places: &Places<'_>, server: &ServerSandbox) -> Confinement {
        let mut confinement = self.confinement(places);
        for path in server.writable.iter().filter_map(|path| owned(path, places.home)) {
            if !confinement.writable.contains(&path) {
                confinement.writable.push(path);
            }
        }
        confinement.network = server.network.unwrap_or(true);
        confinement
    }
}

fn owned(path: &str, home: Option<&Path>) -> Option<PathBuf> {
    match path.strip_prefix("~/") {
        Some(rest) => home.map(|home| home.join(rest)),
        None => Path::new(path).is_absolute().then(|| PathBuf::from(path)),
    }
}

pub fn blocked(output: &str) -> bool {
    output.to_ascii_lowercase().contains("operation not permitted")
}

pub fn hint(settings_dir: &str) -> String {
    format!(
        "(the sandbox may have refused this: commands write only inside the project, added folders, the \
         temporary folder and package caches. Do not work around it; tell the user, who can add a folder to \
         sandbox.allowWrite in ~/{settings_dir}/settings.json)"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn places<'a>(added: &'a [PathBuf], temp: &'a [PathBuf]) -> Places<'a> {
        Places {
            project: Path::new("/repo"),
            added,
            repository: None,
            temp,
            home: Some(Path::new("/Users/ana")),
            settings_dir: ".app",
        }
    }

    #[test]
    fn commands_write_in_the_project_added_folders_temp_and_caches_only() {
        let added = [PathBuf::from("/other")];
        let temp = [PathBuf::from("/tmp")];
        let confinement = Sandbox::default().confinement(&places(&added, &temp));

        assert_eq!(confinement.writable[..3], [PathBuf::from("/repo"), "/other".into(), "/tmp".into()]);
        assert!(confinement.writable.contains(&PathBuf::from("/Users/ana/.cargo/registry")));
        assert!(!confinement.writable.iter().any(|path| path == Path::new("/Users/ana")));
        assert!(!confinement.writable.iter().any(|path| path == Path::new("/Users/ana/.cargo")));
        assert!(confinement.network);
    }

    #[test]
    fn the_settings_that_grant_permissions_stay_out_of_reach() {
        let added = [PathBuf::from("/other")];
        let confinement = Sandbox::default().confinement(&places(&added, &[]));

        assert_eq!(
            confinement.protected,
            [
                PathBuf::from("/repo/.app/settings.json"),
                "/repo/.app/settings.local.json".into(),
                "/other/.app/settings.json".into(),
                "/other/.app/settings.local.json".into(),
            ]
        );
    }

    #[test]
    fn a_session_worktree_reaches_its_repository() {
        let places = Places { repository: Some(Path::new("/repo")), ..places(&[], &[]) };
        let places = Places { project: Path::new("/repo/.app/worktrees/fix"), ..places };

        let confinement = Sandbox::default().confinement(&places);

        assert_eq!(confinement.writable[..2], [PathBuf::from("/repo/.app/worktrees/fix"), "/repo".into()]);
    }

    #[test]
    fn only_the_owner_widens_the_sandbox() {
        let project = json!({ "allowWrite": ["/etc"], "network": false });
        let narrowed = Sandbox::default().layered(&project, false).expect("settings");
        assert!(narrowed.allow_write.is_empty() && !narrowed.network);

        let owners = json!({ "allowWrite": ["~/.cargo", "/data", "relative"], "network": true });
        let widened = Sandbox::default().layered(&owners, true).expect("settings");
        let confinement = widened.confinement(&places(&[], &[]));
        assert!(confinement.writable.contains(&PathBuf::from("/Users/ana/.cargo")));
        assert!(confinement.writable.contains(&PathBuf::from("/data")));
        assert!(!confinement.writable.iter().any(|path| path.ends_with("relative")));

        let off = Sandbox { network: false, ..Sandbox::default() };
        let back_on = off.layered(&json!({ "network": true }), false).expect("settings");
        assert!(!back_on.network, "a project cannot turn the network back on");
    }

    #[test]
    fn the_sandbox_is_required_by_default_where_the_app_has_one_and_not_elsewhere() {
        let has_one = cfg!(any(target_os = "linux", target_os = "macos"));
        assert_eq!(Sandbox::default().required, has_one);
        assert_eq!(REQUIRED_BY_DEFAULT, has_one);
        let read: Sandbox = serde_json::from_value(json!({ "allow_write": [], "network": true })).expect("read");
        assert_eq!(read.required, has_one, "a stored sandbox without the field keeps the default");
    }

    #[test]
    fn requiring_the_sandbox_is_a_restriction_a_project_may_add_but_not_lift() {
        let optional = Sandbox { required: false, ..Sandbox::default() };
        let required = optional.layered(&json!({ "required": true }), false).expect("settings");
        assert!(required.required, "a project may require it");
        let lifted = required.clone().layered(&json!({ "required": false }), false).expect("settings");
        assert!(lifted.required, "a project cannot lift it");
        let owners = required.layered(&json!({ "required": false }), true).expect("settings");
        assert!(!owners.required, "the owner can");
        assert!(Sandbox::default().layered(&json!({ "required": "yes" }), true).is_err());
    }

    #[test]
    fn a_missing_sandbox_is_said_and_refused_only_when_required() {
        let unconfined = Sandbox { required: false, ..Sandbox::default() };
        assert_eq!(unconfined.missing(None), None, "nothing to say where it works");
        let said = unconfined.missing(Some("this kernel has no Landlock")).expect("said");
        assert!(!said.refuses);
        assert!(said.notice.contains("this kernel has no Landlock") && said.notice.contains("unconfined"));
        assert!(said.notice.contains("sandbox.required"), "{}", said.notice);
        let required = Sandbox { required: true, ..Sandbox::default() };
        let refused = required.missing(Some("Windows has no sandbox for commands")).expect("said");
        assert!(refused.refuses && refused.notice.contains("refused"), "{}", refused.notice);
        assert!(refusal("x").contains("sandbox.required") && refusal("x").contains("x"));
    }

    #[test]
    fn a_refusal_says_how_the_user_turns_the_requirement_off() {
        let told = refusal("this kernel has no Landlock");
        let user_settings = format!("~/{}/settings.json", brand::DIR);
        assert!(told.contains(r#""sandbox": { "required": false }"#), "{told}");
        assert!(told.contains(&user_settings), "{told}");
        let required = Sandbox { required: true, ..Sandbox::default() };
        let notice = required.missing(Some("this kernel has no Landlock")).expect("said").notice;
        assert!(notice.contains(r#""required": false"#) && notice.contains(&user_settings), "{notice}");
    }

    #[test]
    fn settings_of_the_wrong_shape_are_refused() {
        assert!(Sandbox::default().layered(&json!({ "network": "yes" }), true).is_err());
        assert!(Sandbox::default().layered(&json!({ "allowWrite": "/data" }), true).is_err());
        assert!(Sandbox::default().layered(&json!({ "allowWrite": [1] }), true).is_err());
    }

    #[test]
    fn an_mcp_server_s_sandbox_is_read_from_its_entry_in_mcp_json() {
        assert_eq!(ServerSandbox::read(&Value::Null), Ok(ServerSandbox::default()));
        assert_eq!(ServerSandbox::read(&json!(true)), Ok(ServerSandbox::default()));
        assert_eq!(ServerSandbox::read(&json!(false)), Ok(ServerSandbox { off: true, ..ServerSandbox::default() }));
        let widened = ServerSandbox::read(&json!({ "writable": ["~/.local/share/notes"], "network": false }));
        assert_eq!(
            widened,
            Ok(ServerSandbox { off: false, writable: vec!["~/.local/share/notes".into()], network: Some(false) })
        );
        assert!(ServerSandbox::read(&json!("off")).is_err());
        assert!(ServerSandbox::read(&json!({ "writable": "/data" })).is_err());
        assert!(ServerSandbox::read(&json!({ "network": "no" })).is_err());
    }

    #[test]
    fn only_the_owner_s_entry_may_turn_a_server_s_sandbox_off_or_widen_it() {
        let asked = ServerSandbox { off: true, writable: vec!["/etc".into()], network: Some(true) };
        assert_eq!(asked.clone().as_declared_by(true), asked);
        let project = asked.as_declared_by(false);
        assert_eq!(project, ServerSandbox { off: false, writable: Vec::new(), network: None });
        let narrowing = ServerSandbox { network: Some(false), ..ServerSandbox::default() };
        assert_eq!(narrowing.clone().as_declared_by(false), narrowing, "a project may still keep it off the network");
    }

    #[test]
    fn an_mcp_server_writes_where_commands_do_plus_its_own_folders_and_keeps_the_network_unless_told() {
        let temp = [PathBuf::from("/tmp")];
        let commands = Sandbox { network: false, allow_write: vec!["/data".into()], ..Sandbox::default() };
        let own = ServerSandbox {
            writable: vec!["~/.local/share/notes".into(), "relative".into()],
            ..ServerSandbox::default()
        };
        let confinement = commands.server_confinement(&places(&[], &temp), &own);
        for folder in ["/repo", "/tmp", "/data", "/Users/ana/.cache", "/Users/ana/.local/share/notes"] {
            assert!(confinement.writable.contains(&PathBuf::from(folder)), "{folder}: {confinement:?}");
        }
        assert!(!confinement.writable.iter().any(|path| path.ends_with("relative")));
        assert!(confinement.network, "MCP servers keep the network by default");
        assert!(confinement.protected.contains(&PathBuf::from("/repo/.app/settings.json")));
        let offline = ServerSandbox { network: Some(false), ..ServerSandbox::default() };
        assert!(!commands.server_confinement(&places(&[], &temp), &offline).network);
    }

    #[test]
    fn a_refused_write_is_recognised_and_explained() {
        assert!(blocked("touch: /etc/x: Operation not permitted"));
        assert!(blocked("zsh:1: operation not permitted: .app/settings.json"));
        assert!(!blocked("No such file or directory"));
        assert!(hint(".app").contains("~/.app/settings.json"));
    }
}
