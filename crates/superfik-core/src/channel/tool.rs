//! The channel tools agents reach over MCP: names, where a run finds them
//! and what the prompt says about them.

use serde::{Deserialize, Serialize};

pub const CHANNELS_SERVER: &str = "superfik_channels";
pub const CHANNEL_CONVERSATIONS_TOOL: &str = "channel_conversations";
pub const CHANNEL_READ_TOOL: &str = "channel_read";
pub const CHANNEL_SEND_TOOL: &str = "channel_send";

const CHANNEL_TOOLS: [&str; 3] = [CHANNEL_CONVERSATIONS_TOOL, CHANNEL_READ_TOOL, CHANNEL_SEND_TOOL];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelTool {
    pub url: String,
    pub token: String,
    #[serde(default)]
    pub drafts: bool,
}

impl ChannelTool {
    pub fn allowed_tools() -> Vec<String> {
        CHANNEL_TOOLS.iter().map(|tool| format!("mcp__{CHANNELS_SERVER}__{tool}")).collect()
    }

    pub fn instructions(&self, accounts: &[String]) -> String {
        let sending = if self.drafts {
            "What you send is kept as a draft for a member to approve; say so in your report."
        } else {
            "What you send goes out to the person, paced like a human typing."
        };
        format!(
            "Channels: you can talk to people outside the organization through {accounts} with the \
             mcp__{CHANNELS_SERVER}__ tools: {CHANNEL_CONVERSATIONS_TOOL} (recent conversations), \
             {CHANNEL_READ_TOOL} (one conversation's messages) and {CHANNEL_SEND_TOOL} (a text reply). \
             {sending} On a LinkedIn page, the conversation `feed` publishes a post; on WhatsApp \
             Business and Instagram, a person can only be answered within 24 hours of their last message. \
             Write as a person of the company would, in the other person's language; never \
             invent facts, prices or dates.",
            accounts = accounts.join(", "),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_are_named_for_claude_code() {
        assert_eq!(ChannelTool::allowed_tools()[2], "mcp__superfik_channels__channel_send");
        let tool = ChannelTool { url: String::new(), token: String::new(), drafts: true };
        let line = tool.instructions(&["WhatsApp · support".into()]);
        assert!(line.contains("WhatsApp · support"));
        assert!(line.contains("draft"));
        let live = ChannelTool { drafts: false, ..tool };
        assert!(live.instructions(&[]).contains("goes out"));
    }
}
