use std::collections::BTreeMap;

use serde_json::Value;

mod backend;
mod learnings;

pub use backend::{CHANNELS, DECLARED_PROVIDERS, MEMORY_SEARCH, PLUGINS, SCHEDULES, TRACE};
pub use learnings::{
    BLAST_RADIUS, CODE_MAP, LEARNING_CHECK, MEMORY_ANCHORS, MEMORY_SCORECARD, MEMORY_SEED, STRUCTURED_LEARNINGS,
};

pub const SANDBOX: &str = "experimental-sandbox";

pub const REDACT_SECRETS: &str = "redact-secrets";
pub const DESTRUCTIVE_SNAPSHOT: &str = "destructive-snapshot";
pub const DEPENDENCY_GUARD: &str = "experimental-dependency-guard";
pub const SUGGESTED_RULES: &str = "suggested-rules";
pub const BUDGET: &str = "budget";
pub const CACHE_WARNING: &str = "cache-warning";
pub const ESCALATION: &str = "experimental-escalation";
pub const PROJECT_CRITERIA: &str = "project-criteria";
pub const AUTO_REVIEW: &str = "auto-review";
pub const FOLLOW_THROUGH: &str = "experimental-follow-through";
pub const AFFECTED_TESTS: &str = "experimental-affected-tests";
pub const PR_FOLLOW: &str = "experimental-pr-follow";
pub const SESSION_CLAIMS: &str = "experimental-session-claims";
pub const LOAD_ON_CALL: &str = "experimental-load-on-call";
pub const OUTPUT_CLEANUP: &str = "experimental-output-cleanup";
pub const OUTPUT_FILTERS: &str = "experimental-output-filters";
pub const OUTPUT_SAVINGS: &str = "experimental-output-savings";
pub const BRANCHES: &str = "branches";
pub const GIT_OFFER: &str = "git-offer";
pub const TEAM_MEMORY: &str = "team-memory";
pub const LEARNING: &str = "experimental-learning";
pub const WEB_MAIN_CONTENT: &str = "experimental-web-main-content";
pub const WEB_CLEANUP: &str = "experimental-web-cleanup";
pub const WEB_LINKS: &str = "experimental-web-links";
pub const WEB_SMALL_PAGES_DIRECT: &str = "experimental-web-small-pages-direct";
pub const WEB_FORMATS: &str = "experimental-web-formats";
pub const WEB_EMPTY_PAGE_ERROR: &str = "experimental-web-empty-page-error";
pub const CODE_MODE: &str = "experimental-code-mode";

pub fn skill_flag(skill: &str) -> Option<&'static str> {
    (skill == "follow-pr").then_some(PR_FOLLOW)
}

const SECURITY: &[&str] = &[SANDBOX, REDACT_SECRETS, DEPENDENCY_GUARD];

pub fn only_restricts(feature: &str, flag: bool) -> bool {
    flag == SECURITY.contains(&feature)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    pub name: &'static str,
    pub about: &'static str,
}

pub const KNOWN: &[Flag] = &[
    Flag {
        name: SANDBOX,
        about: "Confine Bash and Monitor commands to the project's folders (macOS; Linux with Landlock)",
    },
    Flag {
        name: OUTPUT_CLEANUP,
        about: "Bash runs commands without colours and drops escape codes, redrawn progress and repeated lines",
    },
    Flag {
        name: OUTPUT_FILTERS,
        about: "Bash hides passing tests, build steps and progress from cargo, git, npm, pytest and go test, keeping the whole output in a file",
    },
    Flag { name: OUTPUT_SAVINGS, about: "/gain shows how much shorter command output reached the model this session" },
    Flag {
        name: REDACT_SECRETS,
        about: "Hide tokens, keys and the project's .env values in tool output before the model and the log see them",
    },
    Flag {
        name: DESTRUCTIVE_SNAPSHOT,
        about: "Before rm -r, git clean, git reset --hard and the like, keep untracked files so /rewind can bring them back",
    },
    Flag {
        name: DEPENDENCY_GUARD,
        about: "Before npm, cargo or pip installs a package that is missing, brand new or a typo of a popular one, ask you",
    },
    Flag { name: SUGGESTED_RULES, about: "/rules offers permission rules from answers you keep giving" },
    Flag { name: BUDGET, about: "/budget stops a session's model calls once it has spent what you allow" },
    Flag {
        name: CACHE_WARNING,
        about: "The status line counts down the prompt cache's last two minutes and says when it has gone cold",
    },
    Flag {
        name: ESCALATION,
        about: "When the agent fails the same call again and again, send it to the Advisor or to escalationModel",
    },
    Flag {
        name: LEARNING,
        about: "After a run where you corrected the agent, offer one lesson to keep: a memory, a skill or a rule",
    },
    Flag {
        name: TEAM_MEMORY,
        about: concat!(
            "Recall the project's checked-in memories in ",
            brand::dir!(),
            "/memory and share yours there with /memory share <id>"
        ),
    },
    Flag { name: BRANCHES, about: "/branch starts a new conversation from before an earlier prompt, keeping this one" },
    Flag {
        name: GIT_OFFER,
        about: "Offer to initialize git on the first message in a folder that is not a repository",
    },
    Flag {
        name: SESSION_CLAIMS,
        about: "Warn the agent when another session in the same folder edited the file it is changing",
    },
    Flag {
        name: LOAD_ON_CALL,
        about: "A tool waiting behind ToolSearch that the agent calls by name is loaded and run, not refused",
    },
    Flag {
        name: PR_FOLLOW,
        about: "The follow-pr skill watches a pull request and wakes the agent on failing checks and new reviews",
    },
    Flag {
        name: AFFECTED_TESTS,
        about: "After an edit, run the tests that cover the changed file in the background and tell the agent",
    },
    Flag {
        name: WEB_MAIN_CONTENT,
        about: "WebFetch puts a page's main content first and its menus, header and footer after it",
    },
    Flag {
        name: WEB_CLEANUP,
        about: "WebFetch drops hidden parts, buttons, links repeated on every item and long runs of menu links",
    },
    Flag {
        name: WEB_LINKS,
        about: "WebFetch numbers a page's links and lists each address once at the end, same-site ones as paths",
    },
    Flag {
        name: WEB_SMALL_PAGES_DIRECT,
        about: "WebFetch hands a page of 6,000 characters or less to the agent fenced, without the separate reader",
    },
    Flag {
        name: WEB_FORMATS,
        about: "WebFetch asks for Markdown first and compacts JSON (one line per record) and RSS or Atom feeds",
    },
    Flag {
        name: WEB_EMPTY_PAGE_ERROR,
        about: "WebFetch reports a page with almost no text (built by JavaScript, a login, a bot check) as an error",
    },
    Flag {
        name: CODE_MODE,
        about: "REPL becomes the agent's only tool: it writes JavaScript that calls every other tool as a function, and only what the script returns reaches the model",
    },
    Flag {
        name: AUTO_REVIEW,
        about: "When a run changed files, a read-only reviewer checks the changes and the docs before it finishes",
    },
    Flag {
        name: FOLLOW_THROUGH,
        about: "When an answer ends the turn promising more work (\"now I'll run the tests\"), a small model checks it and sends the agent back to do it, twice at most",
    },
    Flag {
        name: PROJECT_CRITERIA,
        about: concat!(
            "Judge every run against the project's checked-in ",
            brand::dir!(),
            "/verification.md when you set no criteria"
        ),
    },
];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Listed {
    pub name: String,
    pub about: String,
    pub on: bool,
    pub experimental: bool,
    pub security: bool,
}

pub fn toggle(argument: Option<&str>) -> Result<Option<(String, bool)>, String> {
    let words: Vec<&str> = argument.unwrap_or_default().split_whitespace().collect();
    match words.as_slice() {
        [] => Ok(None),
        [name, "on"] => Ok(Some(((*name).to_owned(), true))),
        [name, "off"] => Ok(Some(((*name).to_owned(), false))),
        _ => Err("Use /features to list them, or /features <name> on|off.".into()),
    }
}

pub fn known(name: &str) -> Option<Flag> {
    every().find(|flag| flag.name == name)
}

pub fn every() -> impl Iterator<Item = Flag> {
    KNOWN.iter().chain(learnings::KNOWN).chain(backend::KNOWN).copied()
}

pub fn with_flag(mut settings: Value, name: &str, on: bool) -> Result<Value, String> {
    let root = settings.as_object_mut().ok_or("the settings are not a JSON object")?;
    let features = root.entry("features").or_insert_with(|| Value::Object(Default::default()));
    let features = features.as_object_mut().ok_or("features must be an object of true or false")?;
    features.insert(name.to_owned(), Value::Bool(on));
    Ok(settings)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Features {
    set: BTreeMap<String, bool>,
}

impl Features {
    pub fn on(&self, feature: &str) -> bool {
        self.set.get(feature).copied().unwrap_or(true)
    }

    pub fn listed(&self) -> Vec<Listed> {
        every()
            .map(|flag| Listed {
                name: flag.name.to_owned(),
                about: flag.about.to_owned(),
                on: self.on(flag.name),
                experimental: flag.name.starts_with("experimental-"),
                security: SECURITY.contains(&flag.name),
            })
            .collect()
    }

    pub fn off(&self) -> impl Iterator<Item = &str> {
        self.set.iter().filter(|(_, on)| !**on).map(|(name, _)| name.as_str())
    }

    pub fn layered(mut self, value: &Value, owners: bool) -> Result<Self, String> {
        let flags = value.as_object().ok_or("features must be an object of true or false")?;
        for (feature, flag) in flags {
            let flag = flag.as_bool().ok_or_else(|| format!("features.{feature} must be true or false"))?;
            if owners || only_restricts(feature, flag) {
                self.set.insert(feature.clone(), flag);
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn every_feature_is_on_until_the_owner_turns_it_off() {
        let features = Features::default();
        assert!(features.on(SANDBOX) && features.on("insights"));

        let features = features.layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");
        assert!(!features.on(SANDBOX) && !features.on("insights"));
    }

    #[test]
    fn every_flag_code_mode_included_is_on_until_turned_off() {
        let features = Features::default();
        assert!(features.listed().iter().all(|flag| flag.on), "no flag starts off");
        assert!(features.on(CODE_MODE));

        let features = features.layered(&json!({ CODE_MODE: false }), true).expect("flags");
        assert!(!features.on(CODE_MODE));
        assert!(known("experimental-code-mode-only").is_none(), "merged into experimental-code-mode");
    }

    #[test]
    fn a_checked_in_file_only_restricts() {
        let owners = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");
        let back_on = owners.clone().layered(&json!({ "insights": true }), false).expect("flags");
        assert!(!back_on.on("insights"), "a project cannot turn on what the owner turned off");

        let project = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), false).expect("flags");
        assert!(project.on(SANDBOX), "a project cannot turn a security feature off");
        assert!(!project.on("insights"));

        let enforced = owners.layered(&json!({ SANDBOX: true }), false).expect("flags");
        assert!(enforced.on(SANDBOX), "a project may ask for a security feature");
    }

    #[test]
    fn the_known_flags_are_listed_with_their_state() {
        let features = Features::default().layered(&json!({ SANDBOX: false, "insights": false }), true).expect("flags");

        let listed = features.listed();
        let sandbox = listed.iter().find(|flag| flag.name == SANDBOX).expect("sandbox is known");
        assert!(!sandbox.on && sandbox.experimental && sandbox.security);
        assert!(!listed.iter().any(|flag| flag.name == "insights"), "only known flags are listed");
        assert_eq!(features.off().collect::<Vec<_>>(), ["experimental-sandbox", "insights"]);
    }

    #[test]
    fn a_flag_is_written_into_the_settings_keeping_the_rest() {
        let settings = json!({ "model": "x", "features": { "insights": false } });

        let written = with_flag(settings, SANDBOX, false).expect("written");

        assert_eq!(written, json!({ "model": "x", "features": { "insights": false, SANDBOX: false } }));
        assert!(with_flag(json!([]), SANDBOX, true).is_err());
        assert!(with_flag(json!({ "features": 1 }), SANDBOX, true).is_err());
        assert_eq!(known(SANDBOX).map(|flag| flag.name), Some(SANDBOX));
        assert!(known("nothing").is_none());
    }

    #[test]
    fn the_command_lists_or_turns_one_flag_on_or_off() {
        assert_eq!(toggle(None), Ok(None));
        assert_eq!(toggle(Some("  ")), Ok(None));
        assert_eq!(toggle(Some("experimental-sandbox off")), Ok(Some((SANDBOX.into(), false))));
        assert_eq!(toggle(Some("insights on")), Ok(Some(("insights".into(), true))));
        assert!(toggle(Some("insights maybe")).is_err());
        assert!(toggle(Some("insights")).is_err());
    }

    #[test]
    fn a_flag_that_is_not_true_or_false_is_refused() {
        let error = Features::default().layered(&json!({ SANDBOX: "no" }), true).expect_err("refused");
        assert_eq!(error, "features.experimental-sandbox must be true or false");
        assert!(Features::default().layered(&json!([]), true).is_err());
    }
}
