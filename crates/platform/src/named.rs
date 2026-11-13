use std::thread::JoinHandle;

pub fn spawn<T: Send + 'static>(name: &str, work: impl FnOnce() -> T + Send + 'static) -> JoinHandle<T> {
    match std::thread::Builder::new().name(name.to_owned()).spawn(work) {
        Ok(handle) => handle,
        Err(error) => panic!("could not start the {name} thread: {error}"),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_thread_carries_its_name() {
        let name = super::spawn("lsp-sweep", || std::thread::current().name().map(str::to_owned));
        assert_eq!(name.join().ok().flatten().as_deref(), Some("lsp-sweep"));
    }
}
