use crate::{Sandbox, command};

#[test]
fn models_with_no_provider_connected_lists_nothing_and_succeeds() {
    let ran = Sandbox::fresh().run(&["models"]);
    ran.succeeded().quiet_on_stdout().quiet_on_stderr();
}

#[test]
fn logout_with_no_provider_connected_says_so_on_stderr() {
    let ran = Sandbox::fresh().run(&["logout"]);
    ran.succeeded().quiet_on_stdout();
    assert_eq!(ran.stderr, "No provider is connected.\n");
}

#[test]
fn login_lists_the_providers_on_stderr_and_refuses_a_choice_out_of_range() {
    let ran = Sandbox::fresh().run_with_input(&["login"], "99\n");
    ran.failed_with(1).quiet_on_stdout().complained("Connect a provider:\n  1. ");
    assert!(ran.stderr.trim_end().ends_with(&format!("{}: not one of the choices", command())), "{ran:?}");
}

#[test]
fn login_with_nothing_on_stdin_connects_nothing() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["login"]).failed_with(1).quiet_on_stdout().complained("not one of the choices");
    sandbox.run(&["doctor"]).said("none connected");
}
