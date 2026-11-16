use super::*;

#[test]
fn the_process_listening_on_a_port_is_found_with_its_executable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let (pid, executable) = port_holder(port).expect("this test process holds the port");
    assert_eq!(pid, std::process::id());
    assert!(!executable.is_empty());
}

#[test]
fn a_free_port_has_no_holder() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    assert_eq!(port_holder(port), None);
}

#[test]
fn a_terminated_process_ends() {
    let mut child = std::process::Command::new("sleep").arg("30").spawn().expect("sleep");
    terminate(child.id()).expect("terminate");
    let status = child.wait().expect("wait");
    assert!(!status.success());
}
