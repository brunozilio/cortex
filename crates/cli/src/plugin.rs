use app::{Client, PluginsReply, PluginsRequest};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum PluginAction {
    #[command(about = "List the installed plugins")]
    List,
    #[command(about = "Install a plugin from a git URL or path")]
    Install {
        source: String,
        #[arg(long, help = "The folder's name, else taken from the source")]
        name: Option<String>,
    },
    #[command(about = "Pull a plugin's latest commits")]
    Update { name: String },
    #[command(about = "Remove a plugin's folder")]
    Remove { name: String },
}

pub fn run(client: &Client, action: PluginAction) -> Result<(), String> {
    let request = match action {
        PluginAction::List => PluginsRequest::List,
        PluginAction::Install { source, name } => PluginsRequest::Install { source, name },
        PluginAction::Update { name } => PluginsRequest::Update { name },
        PluginAction::Remove { name } => PluginsRequest::Remove { name },
    };
    println!("{}", told(client.plugins(request).map_err(|error| error.to_string())?));
    Ok(())
}

fn told(reply: PluginsReply) -> String {
    match reply {
        PluginsReply::Listed(listed) if listed.is_empty() => {
            format!("No plugins installed. `{} plugin install <git-url>` adds one.", brand::COMMAND)
        }
        PluginsReply::Listed(listed) => listed
            .iter()
            .map(|plugin| format!("{:<24} {}", plugin.name, plugin.description))
            .collect::<Vec<_>>()
            .join("\n"),
        PluginsReply::Installed { name, .. } => format!("Installed {name}. A new chat or /reload picks it up."),
        PluginsReply::Updated { name, git } => format!("Updated {name}: {git}"),
        PluginsReply::Removed { name } => format!("Removed {name}."),
    }
}

#[cfg(test)]
mod tests {
    use app::Plugin;

    use super::*;

    #[test]
    fn each_answer_is_told_in_a_line_of_its_own() {
        let listed = vec![Plugin { name: "ship-it".into(), description: "Ships".into() }];
        assert_eq!(told(PluginsReply::Listed(listed)), format!("{:<24} Ships", "ship-it"));
        assert!(told(PluginsReply::Listed(Vec::new())).starts_with("No plugins installed."));
        let installed = PluginsReply::Installed { name: "ship-it".into(), at: "/home/a/plugins/ship-it".into() };
        assert_eq!(told(installed), "Installed ship-it. A new chat or /reload picks it up.");
        let updated = PluginsReply::Updated { name: "ship-it".into(), git: "Already up to date.".into() };
        assert_eq!(told(updated), "Updated ship-it: Already up to date.");
        assert_eq!(told(PluginsReply::Removed { name: "ship-it".into() }), "Removed ship-it.");
    }
}
