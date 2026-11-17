mod git;

use std::path::Path;

use crate::api::features::workspace::{WorkspaceOp, WorkspaceReply, WorkspaceRequest};

use crate::features::context::Context;

const MENTIONED: usize = 5;
const KEPT: usize = 50_000;

pub fn handle(context: &Context<'_>, request: WorkspaceRequest) -> Result<WorkspaceReply, String> {
    let WorkspaceRequest { path, op } = request;
    match op {
        WorkspaceOp::Changes { scope } => git::changes_in(&path, scope).map(WorkspaceReply::Changes),
        WorkspaceOp::Files => Ok(WorkspaceReply::Files(git::files(&path))),
        WorkspaceOp::GitOffer => {
            let turned_off = !context.on(domain::features::GIT_OFFER);
            Ok(WorkspaceReply::GitOffer { offer: domain::git_offer::due(true, git::inside(&path), turned_off) })
        }
        WorkspaceOp::GitInit => git::init(&path).map(|committed| WorkspaceReply::GitInit { committed }),
        WorkspaceOp::Mentions { prompt } => Ok(WorkspaceReply::Mentions(with_files(&path, &prompt))),
    }
}

pub fn forget(_client: u64) {}

fn with_files(cwd: &Path, prompt: &str) -> String {
    let mut out = prompt.to_owned();
    let mut seen = Vec::new();
    for word in prompt.split_whitespace() {
        let Some(path) = word.strip_prefix('@').map(|p| p.trim_end_matches([',', '.', ':', ';', ')', '?', '!'])) else {
            continue;
        };
        if path.is_empty() || seen.contains(&path) || seen.len() >= MENTIONED {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(cwd.join(path)) else { continue };
        seen.push(path);
        let kept: String = text.chars().take(KEPT).collect();
        let cut = if kept.len() < text.len() { "\n[… cut]" } else { "" };
        out.push_str(&format!("\n\n<file path=\"{path}\">\n{kept}{cut}\n</file>"));
    }
    out
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
