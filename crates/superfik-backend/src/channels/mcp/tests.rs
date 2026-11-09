use std::sync::mpsc;

use superfik_core::channel::Delivery;
use superfik_protocol::ServerEvent;

use super::super::{InMemoryChannelStore, Inbound};
use super::*;

fn tools() -> (ChannelTools, Channels, mpsc::Receiver<(Principal, ServerEvent)>) {
    let (sent, published) = mpsc::channel();
    let sent = Mutex::new(sent);
    let publish: Publish = Arc::new(move |owner, event| {
        let _ = lock(&sent).send((owner.clone(), event));
    });
    let channels = Channels::new(
        Vec::new(),
        std::env::temp_dir(),
        Arc::new(InMemoryChannelStore::default()),
        Arc::new(super::super::sender::SystemClock),
    );
    (ChannelTools::new(channels.clone(), publish), channels, published)
}

fn call(tools: &ChannelTools, token: &str, name: &str, arguments: Value) -> Value {
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } });
    tools.handle(token, &message).ok().flatten().unwrap_or_default()
}

fn text(answer: &Value) -> &str {
    answer["result"]["content"][0]["text"].as_str().unwrap_or_default()
}

fn failed(answer: &Value) -> bool {
    answer["result"]["isError"].as_bool().unwrap_or(false)
}

fn with_a_message(channels: &Channels, account: ConnectionId) {
    let message = Message {
        id: "1".into(),
        exchange: "5511999990000".into(),
        is_group: false,
        sender: Some("5511999990000".into()),
        sender_name: Some("Ana".into()),
        text: "O checkout não abre".into(),
        media: None,
        mentioned_me: false,
        reply_to_me: false,
        at: 10,
        source: Source::Contact,
    };
    let event = superfik_channels::ChannelEvent::Message { message, delivery: Delivery::Live };
    channels.observe(Inbound { owner: Principal::LocalOwner, connection: account, event });
}

#[test]
fn lists_the_three_tools() {
    let (tools, ..) = tools();
    let grant = tools.grant(
        &Principal::LocalOwner,
        RunId::new(),
        "/repo".into(),
        vec![(ConnectionId::new(), "WhatsApp".into())],
        true,
    );
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let answer = tools.handle(&grant.token, &message).ok().flatten().unwrap_or_default();
    let names: Vec<&str> =
        answer["result"]["tools"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, [CHANNEL_CONVERSATIONS_TOOL, CHANNEL_READ_TOOL, CHANNEL_SEND_TOOL]);
}

#[test]
fn an_agent_lists_and_reads_the_conversations_of_its_account() {
    let (tools, channels, _) = tools();
    let account = ConnectionId::new();
    with_a_message(&channels, account);
    let grant = tools.grant(
        &Principal::LocalOwner,
        RunId::new(),
        "/repo".into(),
        vec![(account, "WhatsApp · support".into())],
        true,
    );
    let listed = call(&tools, &grant.token, CHANNEL_CONVERSATIONS_TOOL, json!({}));
    assert_eq!(text(&listed), "`5511999990000` (Ana): Ana: O checkout não abre");
    let read = call(&tools, &grant.token, CHANNEL_READ_TOOL, json!({ "conversation": "5511999990000" }));
    assert_eq!(text(&read), "Ana: O checkout não abre");
    let missing = call(&tools, &grant.token, CHANNEL_READ_TOOL, json!({ "conversation": "123" }));
    assert!(failed(&missing));
}

#[test]
fn a_reply_is_a_draft_the_owner_hears_of() {
    let (tools, channels, published) = tools();
    let account = ConnectionId::new();
    with_a_message(&channels, account);
    let grant =
        tools.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![(account, "WhatsApp".into())], true);
    let sent = call(
        &tools,
        &grant.token,
        CHANNEL_SEND_TOOL,
        json!({ "conversation": "5511999990000", "text": "Oi Ana, já estou vendo." }),
    );
    assert!(text(&sent).contains("draft"));
    assert_eq!(channels.book(&Principal::LocalOwner, account).drafts.len(), 1);
    assert!(matches!(published.try_recv(), Ok((_, ServerEvent::Channel { drafts, .. })) if drafts.len() == 1));
}

#[test]
fn a_live_reply_without_a_link_says_why() {
    let (tools, channels, _) = tools();
    let account = ConnectionId::new();
    with_a_message(&channels, account);
    let grant =
        tools.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![(account, "WhatsApp".into())], false);
    let sent = call(&tools, &grant.token, CHANNEL_SEND_TOOL, json!({ "conversation": "5511999990000", "text": "Oi" }));
    assert!(failed(&sent));
    assert!(text(&sent).contains("not linked"));
}

#[test]
fn with_several_accounts_the_agent_names_one() {
    let (tools, ..) = tools();
    let accounts =
        vec![(ConnectionId::new(), "WhatsApp · sales".into()), (ConnectionId::new(), "WhatsApp · support".into())];
    let grant = tools.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), accounts, true);
    let vague = call(&tools, &grant.token, CHANNEL_CONVERSATIONS_TOOL, json!({}));
    assert!(failed(&vague) && text(&vague).contains("WhatsApp · sales"));
    let named = call(&tools, &grant.token, CHANNEL_CONVERSATIONS_TOOL, json!({ "account": "whatsapp · support" }));
    assert_eq!(text(&named), "No conversations yet.");
}

#[test]
fn a_revoked_token_opens_nothing() {
    let (tools, ..) = tools();
    let grant = tools.grant(&Principal::LocalOwner, RunId::new(), "/repo".into(), vec![], true);
    tools.revoke(&grant.token);
    let message = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    assert_eq!(tools.handle(&grant.token, &message), Err(401));
}
