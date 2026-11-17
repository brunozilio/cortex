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
fn a_blocking_reader_is_told_to_wait_while_nothing_has_arrived() {
    let (mut body, send) = fed(&[]);
    let paused = body.fill_buf().map(<[u8]>::len).map_err(|error| error.kind());
    assert_eq!(paused, Err(io::ErrorKind::TimedOut));
    send.try_send(Ok(b"late".to_vec())).expect("room");
    drop(send);
    let mut text = String::new();
    body.read_to_string(&mut text).expect("text");
    assert_eq!(text, "late");
}
