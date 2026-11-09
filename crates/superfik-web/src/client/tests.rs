use superfik_client::state::{BrowserFocus, EditKey};
use superfik_protocol::ClientCommand;

use super::*;

fn reference() -> WebClient {
    let (session, _) = superfik_client::scenarios::by_name("reference").expect("scenario");
    WebClient::new(session)
}

#[test]
fn it_runs_as_the_web_and_paints_its_hits() {
    let mut client = reference();
    assert_eq!(client.session.platform, Platform::WEB);
    let mut text = TextSystem::new();
    client.paint(0.0, (1280.0, 800.0), 2.0, &HashMap::new(), &mut text);
    client.paint(1.0 / 60.0, (1280.0, 800.0), 2.0, &HashMap::new(), &mut text);
    assert!(client.hits.effort_track.is_some(), "the composer was painted");
}

#[test]
fn typing_reaches_the_composer() {
    let mut client = WebClient::new(Session::default());
    client.key(KeyCommand::Edit(EditKey::Insert("hi".into())), 0.0);
    assert_eq!(client.session.conversation().composer.text(), "hi");
}

#[test]
fn the_servers_settings_take_the_browsers_language_until_one_is_chosen() {
    let mut client = WebClient::new(Session::default());
    let pt = ["pt-BR".to_owned()];
    let settings = serde_json::json!({ "last_workspace": null, "theme": "dark", "onboarded": true });
    client.event(ServerEvent::Settings { settings: Some(settings) }, &pt, 0.0);
    assert_eq!(client.session.preferences.language, Language::Portuguese);
    assert!(client.session.onboarded);
    let chosen = serde_json::json!({ "last_workspace": null, "language": "en" });
    client.event(ServerEvent::Settings { settings: Some(chosen) }, &pt, 0.0);
    assert_eq!(client.session.preferences.language, Language::English);
}

#[test]
fn the_browser_keeps_its_own_shortcuts() {
    let mut client = WebClient::new(Session::default());
    let before = client.session.conversations.len();
    client.key(KeyCommand::Edit(EditKey::Insert("draft".into())), 0.0);
    assert!(client.key(KeyCommand::NewConversation, 0.0).is_empty());
    assert_eq!(client.session.conversations.len(), before);
}

#[test]
fn the_wheel_over_settings_leaves_the_canvas_still() {
    let mut client = WebClient::new(Session::default());
    client.wheel(0.0, 40.0, false, 0.0);
    let panned = client.camera.offset();
    assert!(client.key(KeyCommand::Settings, 0.0).is_empty(), "⌘, stays the browser's");
    client.session.apply(Action::OpenSettings, 0.0);
    client.wheel(0.0, 40.0, false, 0.0);
    assert_eq!(client.camera.offset(), panned);
}

#[test]
fn picked_files_go_up_to_the_server() {
    let mut client = WebClient::new(Session { remote: true, ..Session::default() });
    let io = client.picked(vec![("sky.png".into(), b"sky".to_vec())], 0.0);
    let upload = |io: &Io| matches!(io, Io::Send(c) if matches!(c.as_ref(), ClientCommand::Upload { name, .. } if name == "sky.png"));
    assert!(io.iter().any(upload), "{io:?}");
}

#[test]
fn copy_and_paste_go_through_the_page() {
    let mut client = WebClient::new(Session::default());
    client.key(KeyCommand::Edit(EditKey::Insert("hello".into())), 0.0);
    client.key(KeyCommand::Edit(EditKey::SelectAll), 0.0);
    assert!(matches!(client.key(KeyCommand::Copy, 0.0).as_slice(), [Io::Copy(t)] if t == "hello"));
    let paste = client.key(KeyCommand::Paste, 0.0);
    let [Io::Paste(into)] = paste.as_slice() else { unreachable!("a paste") };
    client.pasted(*into, " world".into(), 0.0);
    assert_eq!(client.session.conversation().composer.text(), " world", "the selection was replaced");
}

fn sent(io: &[Io]) -> Vec<&superfik_core::page::PageCall> {
    io.iter()
        .filter_map(|io| match io {
            Io::Send(command) => match command.as_ref() {
                ClientCommand::Page(call) => Some(call),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn pages_run_on_the_server_and_its_frames_come_back_to_paint() {
    use superfik_core::page::{FrameTile, PageCall, PageFrame, TileCodec};

    let mut client = WebClient::new(Session::default());
    let mut text = TextSystem::new();
    client.paint(0.0, (1280.0, 800.0), 2.0, &HashMap::new(), &mut text);
    let io = client.act([Action::OpenBrowser], 0.0);
    let id = client.session.browser().map(|card| card.id).expect("a card");
    assert!(matches!(sent(&io)[..], [PageCall::Open { scale: 2.0, .. }, ..]), "{io:?}");

    let mut jpeg = Vec::new();
    encoded_tile(&mut jpeg);
    let tile = FrameTile { x: 0, y: 0, w: 8, h: 8, codec: TileCodec::Jpeg, data: jpeg };
    let frame = PageFrame { id, size: (8, 8), whole: true, tiles: vec![tile] };
    let io = client.event(ServerEvent::PageFrame(frame.clone()), &[], 0.0);
    assert!(matches!(&io[..], [Io::PaintPage(paint)] if paint.id == id && paint.tiles.len() == 1));

    client.act([Action::FocusBrowser(Some(BrowserFocus::Page))], 0.0);
    let key = superfik_core::page::KeyInput {
        phase: superfik_core::page::KeyPhase::Down,
        code: 0x41,
        text: Some('a'),
        modifiers: Default::default(),
    };
    let io = client.key_for_page(KeyCommand::Edit(EditKey::Insert("a".into())), Some(key), 0.0);
    assert!(matches!(sent(&io)[..], [PageCall::Input(page, _)] if *page == id));
    assert!(client.session.conversation().composer.text().is_empty(), "the composer typed nothing");

    client.act([Action::CloseBrowser], 0.0);
    assert!(client.event(ServerEvent::PageFrame(frame), &[], 0.0).is_empty());
}

fn encoded_tile(out: &mut Vec<u8>) {
    use superfik_frames::{Order, PageEncoder, Pixels};
    let data = [90u8; 8 * 8 * 4];
    let pixels = Pixels { data: &data, width: 8, height: 8, stride: 32, order: Order::Rgba };
    let frame = superfik_frames::JpegTiles::default().encode(BrowserId::new(), &pixels, true).expect("a frame");
    out.extend_from_slice(&frame.tiles[0].data);
}
