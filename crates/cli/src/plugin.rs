use clap::Subcommand;
use sdk::Sdk;
use serde_json::json;

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

pub fn run(sdk: &Sdk, action: PluginAction) -> Result<(), String> {
    let input = match &action {
        PluginAction::List => json!({ "op": "list" }),
        PluginAction::Install { source, name } => json!({ "op": "install", "source": source, "name": name }),
        PluginAction::Update { name } => json!({ "op": "update", "name": name }),
        PluginAction::Remove { name } => json!({ "op": "remove", "name": name }),
    };
    let answer = sdk.feature("plugins", input).map_err(|error| error.to_string())?;
    match action {
        PluginAction::List => {
            let listed = answer.as_array().cloned().unwrap_or_default();
            if listed.is_empty() {
                println!("No plugins installed. `{} plugin install <git-url>` adds one.", brand::COMMAND);
            }
            for plugin in listed {
                println!(
                    "{:<24} {}",
                    plugin["name"].as_str().unwrap_or_default(),
                    plugin["description"].as_str().unwrap_or_default()
                );
            }
        }
        PluginAction::Install { .. } => {
            println!(
                "Installed {}. A new chat or /reload picks it up.",
                answer["installed"].as_str().unwrap_or_default()
            );
        }
        PluginAction::Update { name } => println!("Updated {name}: {}", answer["git"].as_str().unwrap_or_default()),
        PluginAction::Remove { name } => println!("Removed {name}."),
    }
    Ok(())
}
