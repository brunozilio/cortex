//! Cancellation must interrupt both response headers and a stalled SSE body.
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use superfik_native::model::http::{Http, NetHttp, read_line_stopped};
#[test]
fn cancellation_closes_stalled_connections_before_and_after_headers() {
    for headers in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("valid test fixture");
        let address = listener.local_addr().expect("valid test fixture");
        let (ready_tx, ready_rx) = mpsc::channel();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("valid test fixture");
            socket.set_read_timeout(Some(Duration::from_secs(3))).expect("valid test fixture");
            let mut socket = BufReader::new(socket);
            let mut length = 0;
            loop {
                let mut line = String::new();
                socket.read_line(&mut line).expect("valid test fixture");
                if line.to_ascii_lowercase().starts_with("content-length:") {
                    length = line
                        .split(':')
                        .nth(1)
                        .expect("valid test fixture")
                        .trim()
                        .parse::<usize>()
                        .expect("valid test fixture");
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut data = vec![0; length];
            socket.read_exact(&mut data).expect("valid test fixture");
            if headers {
                socket
                    .get_mut()
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n",
                    )
                    .expect("valid test fixture");
                socket.get_mut().flush().expect("valid test fixture");
            }
            ready_tx.send(()).expect("valid test fixture");
            let mut byte = [0];
            socket.read(&mut byte).expect("valid test fixture") == 0
        });
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let client = std::thread::spawn(move || {
            let mut response = NetHttp.post_stopped(&format!("http://{address}"), &[], "{}", &signal)?;
            read_line_stopped(response.body.as_mut(), &mut String::new(), &signal).map(|_| ())
        });
        ready_rx.recv_timeout(Duration::from_secs(3)).expect("valid test fixture");
        let started = Instant::now();
        stop.store(true, Ordering::Relaxed);
        assert_eq!(client.join().expect("valid test fixture"), Err(superfik_native::model::ModelError::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(server.join().expect("valid test fixture"), "socket must close on cancellation");
    }
}
