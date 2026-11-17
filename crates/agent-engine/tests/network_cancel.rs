use agent_engine::model::http::{Http, NetHttp};
use agent_engine::model::{CancellationToken, ModelError};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    time::Duration,
};

const NEVER_CLOSED: Duration = Duration::from_secs(120);

#[test]
fn cancellation_closes_stalled_connections_before_and_after_headers() {
    for headers in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").expect("valid test fixture");
        let address = listener.local_addr().expect("valid test fixture");
        let (ready_tx, ready_rx) = crossbeam_channel::unbounded();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().expect("valid test fixture");
            socket.set_read_timeout(Some(NEVER_CLOSED)).expect("valid test fixture");
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
            match socket.read(&mut byte) {
                Ok(read) => read == 0,
                Err(error) => error.kind() == std::io::ErrorKind::ConnectionReset,
            }
        });
        let cancel = CancellationToken::new();
        let signal = cancel.clone();
        let client = std::thread::spawn(move || {
            let read = async {
                let mut response = NetHttp.post_cancellable(&format!("http://{address}"), &[], "{}", &signal).await?;
                response.body.read_line(&mut String::new(), &signal).await.map(|_| ())
            };
            platform::runtime::block_on(read).map_err(|error| ModelError::Network(error.to_string()))?
        });
        ready_rx.recv_timeout(NEVER_CLOSED).expect("the server took the request");
        cancel.cancel();
        assert_eq!(client.join().expect("valid test fixture"), Err(agent_engine::model::ModelError::Cancelled));
        assert!(server.join().expect("valid test fixture"), "socket must close on cancellation");
    }
}
