use super::*;

const LONG: &str = "head -c 3000000 /dev/zero | tr '\\0' 'a'; echo; echo the end";
const LONG_BYTES: usize = 3_000_000 + 1 + 8;

fn ended(client: u64) {
    let deadline = Instant::now() + Duration::from_secs(120);
    let read_to_the_end = || {
        let shells = lock();
        let mut mine = shells.values().filter(|shell| shell.client == client);
        mine.all(|shell| shell.readers.iter().all(std::thread::JoinHandle::is_finished))
    };
    while list(client).iter().any(|(_, _, running)| *running) || !read_to_the_end() {
        assert!(Instant::now() < deadline, "the command never ended");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_finished_command_nobody_read_is_forgotten_after_an_hour() {
    start(&of_client(924), "build", &mut sh("echo built")).expect("start");
    ended(924);
    forget_finished_of(Instant::now() + FORGOTTEN_AFTER / 2, Some(924));
    assert_eq!(list(924).len(), 1);
    forget_finished_of(Instant::now() + FORGOTTEN_AFTER * 2, Some(924));
    assert!(list(924).is_empty());
}

#[test]
fn output_that_outgrew_what_is_kept_says_so_before_the_part_that_is_shown() {
    let (stop, detach) = (std::sync::atomic::AtomicBool::new(false), std::sync::atomic::AtomicBool::new(false));
    let id = start(&of_client(921), "long", &mut sh(LONG)).expect("start");
    let Waited::Exited { output, .. } = wait(&id, Duration::from_secs(120), &stop, &detach) else { panic!("ended") };
    let first = output.lines().next().unwrap_or_default();
    assert!(first.starts_with("[earlier output dropped: showing the last "), "{first:.120}");
    assert!(first.ends_with(&format!(" of {LONG_BYTES} bytes]")), "{first:.120}");
    assert!(output.ends_with("the end"));
}

#[test]
fn a_background_read_says_when_output_was_dropped_before_it_was_read() {
    let id = start(&of_client(922), "long", &mut sh(LONG)).expect("start");
    ended(922);
    let text = read(&id, 922);
    let mut lines = text.lines();
    assert!(lines.next().is_some_and(|state| state.contains("finished (exit code 0)")), "{text:.120}");
    let told = lines.next().unwrap_or_default();
    assert!(told.starts_with("[earlier output dropped: showing the last "), "{told:.120}");
    assert!(told.ends_with(&format!(" of {LONG_BYTES} bytes]")), "{told:.120}");
}

#[test]
fn a_short_output_is_shown_as_it_is() {
    let (stop, detach) = (std::sync::atomic::AtomicBool::new(false), std::sync::atomic::AtomicBool::new(false));
    let id = start(&of_client(925), "short", &mut sh("echo all of it")).expect("start");
    let Waited::Exited { output, .. } = wait(&id, Duration::from_secs(120), &stop, &detach) else { panic!("ended") };
    assert_eq!(output, "all of it");
}

#[test]
fn a_finished_command_leaves_the_list_once_its_last_output_was_read() {
    let id = start(&of_client(923), "build", &mut sh("echo built")).expect("start");
    ended(923);
    assert_eq!(list(923).len(), 1, "it stays until it is read");
    let text = read(&id, 923);
    assert!(text.contains("finished") && text.contains("built"), "{text}");
    assert!(list(923).is_empty(), "read to its end, it is forgotten");
    assert!(read(&id, 923).contains("No background command"));
}
