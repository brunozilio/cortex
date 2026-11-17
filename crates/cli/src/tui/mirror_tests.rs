use console::{Mirror, ScreenSize, Shown, Viewer};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::widgets::Paragraph;

use super::{Tee, draw};

fn terminal() -> Terminal<Tee<TestBackend>> {
    Terminal::new(Tee::new(TestBackend::new(20, 3))).expect("terminal")
}

fn say(text: &'static str) -> impl FnOnce(&mut ratatui::Frame) {
    move |frame: &mut ratatui::Frame| {
        frame.render_widget(Paragraph::new(text), frame.area());
        frame.set_cursor_position(Position::new(2, 1));
    }
}

fn screen_of(first: &str) -> [String; 3] {
    [format!("{first:20}"), " ".repeat(20), " ".repeat(20)]
}

fn printed(viewer: &mut Viewer) -> (Vec<ScreenSize>, String) {
    let (mut sizes, mut bytes) = (Vec::new(), Vec::new());
    while let Ok(shown) = viewer.shown.try_recv() {
        match shown {
            Shown::Printed(printed) => bytes.extend(printed),
            Shown::Resized(size) => sizes.push(size),
        }
    }
    (sizes, String::from_utf8_lossy(&bytes).into_owned())
}

#[test]
fn nothing_is_copied_while_no_page_watches() {
    let mirror = Mirror::default();
    let mut terminal = terminal();
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");
    let mut viewer = mirror.join();
    assert_eq!(printed(&mut viewer), (Vec::new(), String::new()));
    terminal.backend().real().assert_buffer_lines(screen_of("hello"));
}

#[test]
fn a_page_that_starts_watching_gets_the_size_then_the_whole_screen_and_the_cursor() {
    let mirror = Mirror::default();
    let mut terminal = terminal();
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");
    let mut viewer = mirror.join();

    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");

    let (sizes, screen) = printed(&mut viewer);
    assert_eq!(sizes, [ScreenSize { cols: 20, rows: 3 }]);
    assert!(screen.starts_with("\x1b[?2004h"), "pastes come wrapped: {screen:?}");
    assert!(screen.contains("\x1b[2J"), "the page's screen is cleared first: {screen:?}");
    assert!(screen.contains("hello"), "the unchanged screen is sent whole: {screen:?}");
    assert!(screen.ends_with("\x1b[2;3H\x1b[?25h"), "the cursor is where the real one is: {screen:?}");
    terminal.backend().real().assert_buffer_lines(screen_of("hello"));
}

#[test]
fn after_the_whole_screen_only_what_changes_is_sent() {
    let mirror = Mirror::default();
    let mut terminal = terminal();
    let mut viewer = mirror.join();
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");
    let _ = printed(&mut viewer);

    draw(&mut terminal, Some(&mirror), say("help")).expect("drawn");

    let (sizes, screen) = printed(&mut viewer);
    assert!(sizes.is_empty());
    assert!(screen.contains('p') && !screen.contains("hel"), "only the changed cells: {screen:?}");
    terminal.backend().real().assert_buffer_lines(screen_of("help"));
}

#[test]
fn a_resized_terminal_sends_its_new_size_and_the_whole_screen_again() {
    let mirror = Mirror::default();
    let mut terminal = terminal();
    let mut viewer = mirror.join();
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");
    let _ = printed(&mut viewer);

    terminal.backend_mut().real.resize(30, 4);
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");

    let (sizes, screen) = printed(&mut viewer);
    assert_eq!(sizes, [ScreenSize { cols: 30, rows: 4 }]);
    assert!(screen.contains("\x1b[2J") && screen.contains("hello"), "{screen:?}");
}

#[test]
fn copying_stops_when_the_last_page_leaves() {
    let mirror = Mirror::default();
    let mut terminal = terminal();
    let viewer = mirror.join();
    draw(&mut terminal, Some(&mirror), say("hello")).expect("drawn");
    drop(viewer);

    draw(&mut terminal, Some(&mirror), say("bye")).expect("drawn");
    assert!(terminal.backend().copy.is_none());

    let mut viewer = mirror.join();
    draw(&mut terminal, Some(&mirror), say("back")).expect("drawn");
    assert!(printed(&mut viewer).1.contains("back"), "a returning page gets the whole screen");
}
