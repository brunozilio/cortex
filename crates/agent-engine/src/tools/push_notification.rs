use std::time::Duration;

use serde_json::{Value, json};
use subprocess::bounded::{Bounds, Ended};

use super::{Access, Outcome, Tool, Workspace, field, required};
use crate::model::ToolSpec;

#[derive(Debug)]
pub struct PushNotification;

pub(crate) fn command(title: &str, message: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    if cfg!(target_os = "macos") {
        let quoted = |text: &str| format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""));
        let script = format!("display notification {} with title {}", quoted(message), quoted(title));
        ("osascript".into(), vec!["-e".into(), script], Vec::new())
    } else if cfg!(windows) {
        let script = concat!(
            "[reflection.assembly]::loadwithpartialname('System.Windows.Forms') | Out-Null; \
             $n = New-Object System.Windows.Forms.NotifyIcon; $n.Icon = [System.Drawing.SystemIcons]::Information; \
             $n.Visible = $true; $n.ShowBalloonTip(5000, $env:",
            brand::env_prefix!(),
            "_TITLE, $env:",
            brand::env_prefix!(),
            "_MESSAGE, 'Info'); \
             Start-Sleep 6"
        );
        let env = vec![
            (concat!(brand::env_prefix!(), "_TITLE").into(), title.into()),
            (concat!(brand::env_prefix!(), "_MESSAGE").into(), message.into()),
        ];
        ("powershell".into(), vec!["-NoProfile".into(), "-Command".into(), script.into()], env)
    } else {
        let args = vec![concat!("--app-name=", brand::name!()).into(), "--".into(), title.into(), message.into()];
        ("notify-send".into(), args, Vec::new())
    }
}

impl Tool for PushNotification {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "PushNotification".into(),
            description: "Show the user a desktop notification: when a long task finishes, when you need a \
                          decision, or when something they asked you to watch happens and they may be away. \
                          Keep it to one short sentence; do not use it for routine progress."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                    "title": { "type": "string", "description": concat!(brand::name!(), " by default.") }
                },
                "required": ["message"],
                "additionalProperties": false
            }),
        }
    }

    fn access(&self) -> Access {
        Access::Reads
    }

    fn summary(&self, input: &Value) -> String {
        field(input, "message").unwrap_or_default().to_owned()
    }

    fn run(&self, input: &Value, workspace: &Workspace) -> Outcome {
        let message = match required(input, "message") {
            Ok(message) => super::clip(message, 300),
            Err(error) => return error,
        };
        let title = field(input, "title").filter(|title| !title.trim().is_empty()).unwrap_or(brand::NAME);
        let title = super::clip(title, 80);
        let (program, args, env) = command(&title, &message);
        let mut process = subprocess::command(&program);
        process.args(&args).envs(env);
        let bounds = Bounds { timeout: Duration::from_secs(15), max_output: 4_096, stop: &workspace.stop, input: None };
        match subprocess::bounded::run(&mut process, bounds) {
            Ok(done) if matches!(done.ended, Ended::Exited(status) if status.success()) => Outcome::ok("Notified."),
            Ok(done) => Outcome::error(format!("The notification failed: {}", done.stderr.trim())),
            Err(_) => Outcome::error(format!("Cannot show notifications here ({program} is missing).")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_message_cannot_break_out_of_the_notification_command() {
        let (program, args, env) = command("Build \"done\"", "it said \"ok\" \\ then; rm -rf /");
        if cfg!(target_os = "macos") {
            assert_eq!(program, "osascript");
            assert_eq!(
                args[1],
                "display notification \"it said \\\"ok\\\" \\\\ then; rm -rf /\" with title \"Build \\\"done\\\"\""
            );
        } else if !cfg!(windows) {
            assert_eq!(program, "notify-send");
            assert_eq!(args[1], "--", "a title like `-u critical` is not an option");
            assert_eq!(args[3], "it said \"ok\" \\ then; rm -rf /", "an argument, never a shell");
        } else {
            assert!(args.iter().all(|arg| !arg.contains("rm -rf")), "the text is never in the script");
            assert_eq!(env[1].1, "it said \"ok\" \\ then; rm -rf /");
        }
    }
}
