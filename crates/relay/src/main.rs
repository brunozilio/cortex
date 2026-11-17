use std::net::SocketAddr;

const LISTEN: &str = "RELAY_LISTEN";
const DEFAULT_LISTEN: &str = "0.0.0.0:8080";

fn listen_address() -> Result<SocketAddr, String> {
    let mut arguments = std::env::args().skip(1);
    let asked = match (arguments.next().as_deref(), arguments.next()) {
        (Some("--listen"), Some(address)) => address,
        (None, _) => std::env::var(LISTEN).unwrap_or_else(|_| DEFAULT_LISTEN.to_owned()),
        _ => return Err(format!("usage: relay [--listen <address:port>]  (or {LISTEN}; default {DEFAULT_LISTEN})")),
    };
    asked.parse().map_err(|error| format!("`{asked}` is not an address and port: {error}"))
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let address = match listen_address() {
        Ok(address) => address,
        Err(problem) => {
            eprintln!("{problem}");
            return std::process::ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("could not listen on {address}: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if !page::has_index(page::embedded()) {
        eprintln!("this relay was built without the page; phones will see a notice until it is rebuilt");
    }
    eprintln!("relay listening on {address}; put TLS in front of it");
    let app = relay::router(relay::Limits::default(), page::embedded());
    let stopped = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    match axum::serve(listener, app).with_graceful_shutdown(stopped).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("the relay stopped: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
