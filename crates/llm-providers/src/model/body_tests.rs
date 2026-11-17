use std::time::{Duration, Instant};

use super::*;

fn run<T: Send>(future: impl Future<Output = T> + Send) -> T {
    platform::runtime::block_on(future).expect("runtime")
}

fn fed(chunks: &[&[u8]]) -> (Body, mpsc::Sender<Chunk>) {
    let (send, receive) = mpsc::channel(16);
    for chunk in chunks {
        send.try_send(Ok(chunk.to_vec())).expect("room");
    }
    (Body::chunks(receive), send)
}

#[test]
fn a_character_split_across_two_chunks_arrives_whole() {
    let text = "data: café\nnext\n".as_bytes();
    let (head, tail) = text.split_at(10);
    let (mut body, send) = fed(&[head, tail]);
    drop(send);
    let never = CancellationToken::new();
    let mut line = String::new();
    assert_eq!(run(body.read_line(&mut line, &never)), Ok(12));
    assert_eq!(line, "data: café\n");
    line.clear();
    assert_eq!(run(body.read_line(&mut line, &never)), Ok(5));
    assert_eq!(run(body.read_line(&mut String::new(), &never)), Ok(0));
}

#[test]
fn a_stream_that_sends_nothing_for_its_idle_deadline_is_a_typed_timeout() {
    let (body, _held_open) = fed(&[b"data: a\n"]);
    let mut body = body.idle(Duration::from_millis(40));
    let never = CancellationToken::new();
    let mut line = String::new();
    assert_eq!(run(body.read_line(&mut line, &never)), Ok(8));
    let started = Instant::now();
    assert_eq!(run(body.read_line(&mut line, &never)), Err(ModelError::Stalled(0)));
    assert!(started.elapsed() >= Duration::from_millis(40));
    assert!(ModelError::Stalled(0).is_transient());
}

#[test]
fn cancelling_wakes_a_read_that_waits_for_bytes_at_once() {
    let (mut body, _held_open) = fed(&[]);
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        canceller.cancel();
    });
    let started = Instant::now();
    assert_eq!(run(body.read_line(&mut String::new(), &cancel)), Err(ModelError::Cancelled));
    assert!(started.elapsed() < Duration::from_millis(500), "{:?}", started.elapsed());
}

#[test]
fn a_bounded_body_refuses_more_than_its_limit() {
    let (body, send) = fed(&[b"12345", b"67890"]);
    drop(send);
    let mut body = body.bounded(7);
    let never = CancellationToken::new();
    let mut first = [0; 5];
    assert_eq!(run(body.read_full(&mut first, &never)), Ok(5));
    let refused = run(body.read_full(&mut [0; 5], &never));
    assert_eq!(refused, Err(ModelError::Network(TOO_LONG.into())));
    let text = run(Body::from("too long").text(&never, 3));
    assert!(matches!(text, Err(ModelError::Provider(_))), "{text:?}");
}

#[test]
fn a_blocking_reader_waits_for_what_arrives_and_wakes_with_it() {
    let (mut body, send) = fed(&[]);
    let (sent_at, sent) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        let _ = sent_at.send(Instant::now());
        send.try_send(Ok(b"late".to_vec())).expect("room");
    });
    let mut text = String::new();
    body.read_to_string(&mut text).expect("text");
    let woke = Instant::now();
    assert_eq!(text, "late");
    let sent = sent.recv().expect("sent");
    assert!(woke - sent < Duration::from_millis(20), "{:?}", woke - sent);
}

#[test]
fn a_capped_read_stops_at_the_cap_and_a_short_body_is_read_whole() {
    let never = CancellationToken::new();
    assert_eq!(run(Body::from("hello").capped(&never, 1024)), Ok(b"hello".to_vec()));
    assert_eq!(run(Body::from("hello").capped(&never, 3)), Ok(b"hel".to_vec()));
    assert_eq!(run(Body::from("hello").capped(&never, 0)), Ok(Vec::new()));
    let endless = Body::reader(io::BufReader::new(io::repeat(b'a')));
    assert_eq!(run(endless.capped(&never, 100_000)).map(|bytes| bytes.len()), Ok(100_000));
}

#[test]
fn a_capped_read_waiting_for_bytes_ends_when_cancelled() {
    let (body, _held_open) = fed(&[b"some"]);
    let cancel = CancellationToken::new();
    let canceller = cancel.clone();
    let (marked, cancelled_at) = crossbeam_channel::unbounded();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        let _ = marked.send(Instant::now());
        canceller.cancel();
    });
    assert_eq!(run(body.capped(&cancel, 1024)), Err(ModelError::Cancelled));
    let ended = Instant::now();
    let cancelled = cancelled_at.recv().expect("cancelled");
    assert!(ended - cancelled < Duration::from_secs(1), "{:?}", ended - cancelled);
}

#[test]
fn a_line_that_never_ends_is_refused_at_its_cap_instead_of_filling_memory() {
    let (body, feeding) = fed(&[]);
    let mut body = body;
    let feeder = std::thread::spawn(move || {
        let chunk = vec![b'a'; 64 * 1024];
        let mut sent = 0usize;
        while feeding.blocking_send(Ok(chunk.clone())).is_ok() && sent < 64 * 1024 * 1024 {
            sent += chunk.len();
        }
        sent
    });
    let mut line = String::new();
    let read = run(body.read_line_within(&mut line, &CancellationToken::new(), 256 * 1024));
    assert_eq!(read, Err(ModelError::Oversized(256 * 1024)));
    assert!(line.is_empty());
    drop(body);
    assert!(feeder.join().expect("feeder") < 64 * 1024 * 1024, "reading stopped at the cap");
    assert!(!ModelError::Oversized(1).is_transient());
    assert_eq!(LINE_AT_MOST, 8 * 1024 * 1024);
}

#[test]
fn a_line_just_under_the_cap_still_reads() {
    let text = format!("{}\n", "b".repeat(1023));
    let mut body = Body::from(text.as_str());
    let mut line = String::new();
    assert_eq!(run(body.read_line_within(&mut line, &CancellationToken::new(), 1024)), Ok(1024));
    assert_eq!(line, text);
}
