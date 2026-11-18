use super::Flag;

pub const MEMORY_SEARCH: &str = "memory-search";
pub const SCHEDULES: &str = "schedules";
pub const PLUGINS: &str = "plugins";
pub const TRACE: &str = "trace";
pub const CHANNELS: &str = "channels";
pub const DECLARED_PROVIDERS: &str = "declared-providers";

pub const KNOWN: &[Flag] = &[
    Flag { name: MEMORY_SEARCH, about: "The memory_search tool the agent uses to look further into project memory" },
    Flag { name: SCHEDULES, about: "/loop and /schedules send a prompt again later or on an interval" },
    Flag { name: PLUGINS, about: "Install, update, list and remove plugins" },
    Flag { name: TRACE, about: "/trace and /console show what each run did, on the local web page" },
    Flag { name: CHANNELS, about: "MCP servers that stay open for the session and send messages to the agent" },
    Flag {
        name: DECLARED_PROVIDERS,
        about: "Load the model providers that files in your settings folder and the project declare",
    },
];

#[cfg(test)]
mod tests {
    use super::super::{Features, known};
    use super::*;

    #[test]
    fn a_backend_feature_s_switch_is_a_known_flag_that_is_on_until_turned_off() {
        let listed = Features::default().listed();
        for flag in KNOWN {
            assert_eq!(known(flag.name), Some(*flag));
            assert!(!flag.name.contains('_'), "{}: flags are written with hyphens", flag.name);
            let shown = listed.iter().find(|listed| listed.name == flag.name).expect("listed");
            assert!(shown.on && !shown.experimental && !shown.security, "{}", flag.name);
        }
        let off = Features::default().layered(&serde_json::json!({ TRACE: false }), true).expect("flags");
        assert!(!off.on(TRACE) && off.on(PLUGINS));
    }

    #[test]
    fn memory_has_no_switch_and_an_old_setting_that_turned_it_off_still_loads() {
        assert!(known("memory").is_none());
        let old = Features::default().layered(&serde_json::json!({ "memory": false }), true).expect("flags");
        assert!(old.listed().iter().all(|flag| flag.on));
    }
}
