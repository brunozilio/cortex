fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("--mcp") => superfik_native::worker::mcp_main(),
        Some("--brief") => superfik_native::brief::worker_main(),
        _ => superfik_native::worker::main(),
    };
    if let Err(error) = result {
        eprintln!("native worker: {error}");
        std::process::exit(1);
    }
}
