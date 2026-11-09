//! The notes tools agents reach over MCP: names, where a run finds them
//! and what the prompt says about them.

use serde::{Deserialize, Serialize};

pub const NOTES_SERVER: &str = "superfik_notes";
pub const NOTES_LIST_TOOL: &str = "notes_list";
pub const NOTES_READ_TOOL: &str = "notes_read";
pub const NOTES_WRITE_TOOL: &str = "notes_write";
pub const NOTES_CHECK_TOOL: &str = "notes_check";

const NOTES_TOOLS: [&str; 4] = [NOTES_LIST_TOOL, NOTES_READ_TOOL, NOTES_WRITE_TOOL, NOTES_CHECK_TOOL];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotesTool {
    pub url: String,
    pub token: String,
}

impl NotesTool {
    pub fn allowed_tools() -> Vec<String> {
        NOTES_TOOLS.iter().map(|tool| format!("mcp__{NOTES_SERVER}__{tool}")).collect()
    }

    pub fn instructions() -> String {
        format!(
            "Project notes: the user's sticky notes for this project (a title line, text, `- [ ] item` \
             checklists). Use the mcp__{NOTES_SERVER}__ tools when the user asks about them or to keep one up \
             to date: {NOTES_LIST_TOOL}, {NOTES_READ_TOOL}, {NOTES_WRITE_TOOL} (a new note, linked to this \
             session, or a whole new body for one) and {NOTES_CHECK_TOOL} (tick a checklist item)."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_are_named_for_claude_code() {
        assert_eq!(NotesTool::allowed_tools()[0], "mcp__superfik_notes__notes_list");
        assert_eq!(NotesTool::allowed_tools().len(), 4);
        assert!(NotesTool::instructions().contains("mcp__superfik_notes__"));
    }
}
