use crate::{Sandbox, command};

const SUBCOMMANDS: [&str; 8] = ["login", "logout", "models", "mcp", "plugin", "doctor", "eval", "config"];

#[test]
fn version_prints_the_command_and_its_version_on_stdout() {
    let ran = Sandbox::fresh().run(&["--version"]);
    ran.succeeded().quiet_on_stderr();
    assert_eq!(ran.stdout.trim(), format!("{} 0.0.1", command()));
    let short = Sandbox::fresh().run(&["-V"]);
    assert_eq!(short.stdout, ran.stdout);
}

#[test]
fn help_lists_every_subcommand_and_the_print_options() {
    let ran = Sandbox::fresh().run(&["--help"]);
    ran.succeeded().quiet_on_stderr();
    ran.said(&format!("Usage: {} [OPTIONS] [PROMPT]... [COMMAND]", command()));
    for name in SUBCOMMANDS {
        assert!(ran.stdout.lines().any(|line| line.trim_start().starts_with(name)), "{name} is not listed: {ran:?}");
    }
    for option in ["--print", "--output-format", "--input-format", "--trust", "--max-turns", "--cwd"] {
        ran.said(option);
    }
}

#[test]
fn every_help_text_names_the_real_folder_and_no_placeholder_is_left() {
    let sandbox = Sandbox::fresh();
    let mut pages = vec![sandbox.run(&["--help"])];
    pages.extend(SUBCOMMANDS.iter().map(|name| sandbox.run(&[name, "--help"])));
    for page in &pages {
        page.succeeded().quiet_on_stderr();
        assert!(!page.stdout.contains("{command}"), "a placeholder was printed as is: {page:?}");
    }
    pages[0].said(&format!("~/{}/plugins", brand::DIR)).said(&format!("{}/evals", brand::DIR));
    sandbox.run(&["config", "--help"]).said(&format!("{}/settings.local.json", brand::DIR));
}

#[test]
fn an_unknown_option_is_a_usage_error_on_stderr_with_exit_code_2() {
    let ran = Sandbox::fresh().run(&["--no-such-option"]);
    ran.failed_with(2).quiet_on_stdout().complained("unexpected argument '--no-such-option'");
}

#[test]
fn an_unknown_subcommand_of_a_group_is_a_usage_error() {
    let sandbox = Sandbox::fresh();
    sandbox.run(&["mcp", "frobnicate"]).failed_with(2).quiet_on_stdout().complained("unrecognized subcommand");
    sandbox.run(&["plugin", "frobnicate"]).failed_with(2).quiet_on_stdout().complained("unrecognized subcommand");
    sandbox.run(&["config", "frobnicate"]).failed_with(2).quiet_on_stdout().complained("unrecognized subcommand");
}

#[test]
fn a_prompt_without_a_terminal_or_print_mode_fails_early_without_drawing() {
    let sandbox = Sandbox::fresh();
    for args in [vec!["bogus-sub"], vec![], vec!["--inline", "hello there"]] {
        let ran = sandbox.run(&args);
        ran.failed_with(2).quiet_on_stdout().complained("no terminal: use -p for print mode");
    }
}

#[test]
fn a_project_folder_that_does_not_exist_fails_naming_it() {
    let sandbox = Sandbox::fresh();
    let missing = sandbox.path("missing");
    let shown = missing.display().to_string();
    for args in [vec!["-C", &shown, "doctor"], vec!["--cwd", &shown, "-p", "hi"], vec!["-C", &shown, "config", "list"]]
    {
        let ran = sandbox.run(&args);
        ran.failed_with(1).quiet_on_stdout().complained(&format!("{}: {shown}", command()));
    }
}

#[test]
fn the_cwd_option_points_the_command_at_another_project() {
    let sandbox = Sandbox::fresh();
    let elsewhere = sandbox.path("elsewhere");
    std::fs::create_dir(&elsewhere).expect("folder");
    let shown = elsewhere.display().to_string();
    sandbox.run(&["-C", &shown, "config", "--project", "set", "model", "m"]).succeeded();
    assert!(elsewhere.join(brand::DIR).join("settings.local.json").is_file());
    assert!(!sandbox.project_dir().exists(), "the current folder is left alone");
}
