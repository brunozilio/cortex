use super::*;

fn plain(command: &str) -> CommandClass {
    classify(command, &GitFacts::default(), &nowhere)
}

#[test]
fn commands_that_only_look_inside_the_project_are_read_only() {
    for command in [
        "ls -la",
        "git status",
        "git log --oneline -5",
        "git diff HEAD -- src/lib.rs",
        "cat src/main.rs | grep fn | wc -l",
        "rg -n 'todo' crates && git branch -a",
        "find . -name main.rs",
        "pwd; which cargo",
    ] {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn writing_reaching_outside_or_hiding_a_command_is_left_to_the_usual_checks() {
    for command in [
        "ls > listing.txt",
        "cat /etc/hosts",
        "cat ~/notes.md",
        "cat ../other/README.md",
        "find . -name '*.tmp' -delete",
        "find . -exec rm {} ;",
        "git branch -D old",
        "git log --output=log.txt",
        "ls $(pwd)",
        "cat `ls`",
        "sort -o out.txt in.txt",
        "cargo build",
        "ls && touch made",
        "grep 'unclosed",
        "",
    ] {
        assert_eq!(plain(command), CommandClass::Unknown, "{command}");
    }
}

fn read_only(commands: &[&str]) -> Vec<String> {
    commands.iter().filter(|command| plain(command) == CommandClass::ReadOnly).map(|c| (*c).to_owned()).collect()
}

#[test]
fn a_changed_environment_or_a_wrapper_is_never_read_only() {
    let slipped = read_only(&[
        "LD_PRELOAD=./x.so ls",
        "PATH=./bin ls",
        "GIT_EXTERNAL_DIFF=./x git diff",
        "sudo cat notes.txt",
        "env ls",
        "nice ls",
        "doas ls",
        "command ls",
        "builtin pwd",
        "nohup ls",
        "time ls",
        "./ls",
        "bin/ls -la",
        "( ls )",
        "! ls",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
}

#[test]
fn a_git_read_that_checks_a_signature_runs_the_signing_program_so_it_is_never_read_only() {
    let slipped = read_only(&[
        "git log --format=%GS",
        "git log --format=%GK -1",
        "git log --pretty=format:%GG",
        "git log --pretty=tformat:%h%GF",
        "git log --format=%GP",
        "git log --format=%GT",
        "git log '--format=%h %G?'",
        "git log --format='%an %GS'",
        "git log \"--pretty=format:%GK\"",
        "git show --pretty=format:%GK -s",
        "git show -s --format=%GG HEAD",
        "git log --show-signature",
        "git show --show-signature HEAD",
        "git branch --format='%(signature)'",
        "git tag -l --format=%(signature:key)",
        "git tag -v v1",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
    for command in
        ["git log --format='%h %s'", "git log --pretty=oneline", "git log -G pattern", "git log --format=%gs"]
    {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn a_flag_that_writes_or_runs_a_program_is_never_read_only() {
    let slipped = read_only(&[
        "rg --pre ./script pattern",
        "sort --output=out.txt x",
        "sort -o out.txt x",
        "sort --compress-program=./x a",
        "rg -z todo",
        "rg --hostname-bin=./x todo",
        "uniq in out",
        "tree -o out",
        "git grep --open-files-in-pager=cmd x",
        "git grep -Ocmd x",
        "git diff --ext-diff",
        "git log --output=log.txt",
        "git branch new-branch",
        "git tag -l -d v1",
        "git tag v2",
        "file -C -m magic",
        "tail -f notes.txt",
        "ls --unknown-flag",
        "jq env",
        "find . -fprint out",
        "find . -newerXY",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
}

#[test]
fn a_word_the_shell_would_expand_or_that_leaves_the_project_is_never_read_only() {
    let slipped = read_only(&[
        "cat $HOME/.aws/config",
        "cat \\/etc/hosts",
        "cat .en?",
        "cat *.txt",
        "cat [a-z]otes.txt",
        "cat {a,b}.txt",
        "cat ~/notes.md",
        "cat x~",
        "cat \"/etc/hosts\"",
        "cat '/etc'/hosts",
        "cat </etc/hosts",
        "cat src/../../x",
        "grep -r key --include=/etc/x",
        "head -n5 \"../x\"",
        "git diff HEAD~",
        "find . -name *.rs",
        "grep \"$HOME\" notes.txt",
        "grep a\\*b notes.txt",
        "cat <<EOF\nx\nEOF",
        "ls 'unclosed",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
}

#[test]
fn a_quoted_pattern_or_a_revision_s_tilde_is_an_argument_the_shell_leaves_alone() {
    for command in ["find . -name '*.rs'", "grep 'a.*b' notes.txt", "git diff HEAD~1", "rg -n \"todo|fixme\" src"] {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn a_control_structure_a_function_or_a_substitution_is_never_read_only() {
    let slipped = read_only(&[
        "if true; then ls; fi",
        "for f in a b; do cat $f; done",
        "while true; do ls; done",
        "case x in x) ls;; esac",
        "f() { ls; }; f",
        "{ ls; }",
        "ls $(pwd)",
        "echo `ls`",
        "[[ -f a ]]",
        "ls | xargs cat",
        "busybox cat notes.txt",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
}

#[test]
fn a_secret_is_found_inside_any_command_of_the_line() {
    for command in [
        "echo $(cat .env)",
        "if true; then cat .env; fi",
        "ls | xargs cat .env",
        "find . -name x -exec cat .env \\;",
        "f() { cat ~/.ssh/id_rsa; }",
        "sudo cat .env",
        "env X=1 cat .env",
    ] {
        assert!(matches!(plain(command), CommandClass::Forbidden(_)), "{command}: {:?}", plain(command));
    }
}

#[test]
fn a_risky_command_is_found_inside_any_command_of_the_line() {
    for command in ["if true; then git push --force; fi", "echo $(cargo publish)", "sudo gh pr merge 1"] {
        assert!(risky(command, &on("feature")), "{command}");
    }
}

#[test]
fn only_a_listed_reader_may_describe_itself_without_asking() {
    let slipped = read_only(&["docker --version", "cargo --help", "deploy --help", "X=1 ls --help", "./ls --version"]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
    let cmd = brand::COMMAND;
    for command in ["ls --help".to_owned(), "git --version".to_owned(), format!("{cmd} --version")] {
        assert_eq!(plain(&command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn a_listed_reader_with_listed_flags_and_plain_words_is_read_only() {
    for command in [
        "ls",
        "ls -la src",
        "ls -1 --color=never",
        "cat -n notes.txt",
        "head -5 notes.txt",
        "head -n 5 notes.txt",
        "tail -n20 notes.txt 2>/dev/null",
        "wc -l src/main.rs",
        "grep -rn --include=lib.rs \"fn main\" src",
        "grep -A3 -B 2 -e todo notes.txt",
        "cut -d: -f1 notes.txt",
        "diff -u a.txt b.txt",
        "du -sh src",
        "stat notes.txt",
        "find . -name main.rs -type f",
        "find src -maxdepth 2 -iname readme.md",
        "git status --short",
        "git log --oneline -5",
        "git log -n 3 --stat",
        "git diff --cached -- src/lib.rs",
        "git show HEAD --stat",
        "git blame -L 1,5 src/lib.rs",
        "git grep -n todo",
        "git ls-files -o --exclude-standard",
        "git rev-parse --abbrev-ref HEAD",
        "git branch -a",
        "git branch --show-current",
        "git tag --list",
        "git remote -v",
        "pwd; which cargo",
        "date -u +%Y-%m-%d",
        "sort -rn notes.txt",
        "rg -n 'todo' crates",
        "rg --files -g Cargo.toml",
    ] {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
}

#[test]
fn dumping_bytes_counting_lines_or_translating_characters_only_reads() {
    for command in [
        "strings app.exe | grep -iE 'login|token' | head -100",
        "strings -n 8 -a app.exe",
        "hexdump -C app.exe | head -20",
        "od -A x -t x1z -N 64 app.exe",
        "sort notes.txt | uniq -c | sort -rn",
        "cut -d: -f1 notes.txt | tr -d ' ' | tr a-z A-Z",
    ] {
        assert_eq!(plain(command), CommandClass::ReadOnly, "{command}");
    }
    let slipped = read_only(&[
        "uniq notes.txt out.txt",
        "hexdump -f format.txt app.exe",
        "strings -T bfd app.exe",
        "xxd app.exe out.hex",
        "xxd -r dump.hex app.exe",
        "awk 'BEGIN { system(\"id\") }'",
    ]);
    assert!(slipped.is_empty(), "read-only by mistake: {slipped:?}");
}

#[test]
fn printing_secrets_is_refused_whatever_else_the_command_does() {
    for command in [
        "cat .env",
        "head -5 config/.env.production",
        "grep KEY .env.local",
        "base64 ~/.ssh/id_ed25519",
        "cargo build && cat secrets/server.pem",
        "printenv",
        "env",
        "printenv OPENAI_API_KEY",
        "cp .npmrc leaked.txt",
    ] {
        assert!(matches!(plain(command), CommandClass::Forbidden(_)), "{command}: {:?}", plain(command));
    }
    for command in ["ls .env", "ls -a", "env CARGO_TERM_COLOR=always cargo test", "cat .env.example", "git status"] {
        assert!(!matches!(plain(command), CommandClass::Forbidden(_)), "{command}");
    }
}

#[test]
fn a_wildcard_that_could_name_a_secret_file_is_refused_like_the_file_itself() {
    for command in [
        "cat .env*",
        "cat .en?",
        "cat ~/.ssh/id_*",
        "cat .e[n]v",
        "head -3 config/*.pem",
        "grep KEY .env.*",
        "cat .{env,envrc}",
        "cat .en\\v",
        "cat secrets.j*",
        "cat *",
    ] {
        assert!(matches!(plain(command), CommandClass::Forbidden(_)), "{command}: {:?}", plain(command));
    }
    for command in ["cat *.txt", "cat src/*.rs", "grep -r key docs/*.md", "cat notes.{md,txt}", "ls .env*"] {
        assert!(!matches!(plain(command), CommandClass::Forbidden(_)), "{command}");
    }
}

#[test]
fn loading_a_secret_file_into_the_shell_is_refused_like_printing_it() {
    for command in ["source .env", ". .env", ". ./config/.env.production && cargo run", "source ~/.ssh/id_rsa"] {
        assert!(matches!(plain(command), CommandClass::Forbidden(_)), "{command}: {:?}", plain(command));
    }
    for command in ["source venv/bin/activate", ". ./setup.sh", "source .env.example"] {
        assert!(!matches!(plain(command), CommandClass::Forbidden(_)), "{command}");
    }
}

fn on(branch: &str) -> GitFacts<'_> {
    GitFacts { current_branch: Some(branch), default_branch: Some("main") }
}

fn risky(command: &str, git: &GitFacts<'_>) -> bool {
    matches!(classify(command, git, &nowhere), CommandClass::Risky(_))
}

#[test]
fn rewriting_or_deleting_shared_history_is_always_asked_about() {
    let feature = on("feature");
    for command in [
        "git push --force",
        "git push -f origin feature",
        "git push --force-with-lease",
        "git push origin +feature",
        "git push origin --delete old",
        "git push origin :old",
        "git push --mirror",
        "cargo test && git push -f",
    ] {
        assert!(risky(command, &feature), "{command}");
    }
    assert!(!risky("git push", &feature));
    assert!(!risky("git push -u origin feature", &feature));
}

#[test]
fn pushing_to_the_default_branch_is_asked_about() {
    assert!(risky("git push", &on("main")));
    assert!(risky("git push origin main", &on("feature")));
    assert!(risky("git push origin HEAD:main", &on("feature")));
    assert!(!risky("git push origin feature", &on("feature")));
    let unknown = GitFacts { current_branch: Some("master"), default_branch: None };
    assert!(risky("git push", &unknown), "main and master count as default when git does not say");
}

#[test]
fn publishing_and_merging_are_asked_about() {
    for command in [
        "cargo publish",
        "npm publish --access public",
        "gh pr merge 12 --squash",
        "gh release create v1.0",
        "gh api -X DELETE repos/o/r",
        "docker push org/image:latest",
    ] {
        assert!(risky(command, &GitFacts::default()), "{command}");
    }
    assert!(!risky("gh pr view 12", &GitFacts::default()));
}

#[test]
fn global_options_before_the_subcommand_do_not_hide_a_risky_command() {
    let feature = on("feature");
    for command in [
        "git -C . push --force origin main",
        "git -C repo push -f",
        "git --git-dir=.git push -f",
        "git --git-dir .git --work-tree . push --force",
        "git -c push.default=current push --force",
        "git --no-pager -P --bare push --mirror",
        "git --namespace=x --exec-path=/usr/lib/git push origin :old",
        "git --literal-pathspecs --no-optional-locks push origin main",
        "git --unknown-flag value push -f",
        "npm --prefix . publish",
        "npm -C . --registry https://r.dev publish",
        "npm --workspace pkg --loglevel silent publish",
        "pnpm --filter pkg publish",
        "pnpm -r publish",
        "yarn --cwd pkg publish",
        "yarn workspace pkg npm publish",
        "yarn npm publish",
        "cargo +nightly publish",
        "cargo --locked -Z unstable-options publish",
        "cargo --config net.offline=true publish",
        "cargo -C crate publish",
        "gh -R owner/repo release create v1",
        "gh --repo owner/repo pr merge 3",
        "docker --context remote push org/image",
        "docker -H tcp://h:2375 push org/image",
        "podman --log-level debug push org/image",
    ] {
        assert!(risky(command, &feature), "{command}");
    }
    for command in ["git -C . status", "git -c core.pager=cat log", "npm --prefix . install", "cargo +nightly build"] {
        assert!(!risky(command, &feature), "{command}");
    }
}

#[test]
fn signing_in_to_an_mcp_server_is_refused_inside_the_sandbox_only() {
    let cmd = brand::COMMAND;
    for command in [
        format!("{cmd} mcp login jira"),
        format!("{cmd} mcp login jira; {cmd} mcp login github"),
        format!("/Users/me/.cargo/bin/{cmd} mcp login cloudflare"),
    ] {
        let why = sign_in_refused(&command, true).unwrap_or_else(|| panic!("{command}"));
        assert!(why.contains("unsandboxed") && why.contains("/mcp login"), "{why}");
        assert_eq!(sign_in_refused(&command, false), None, "outside the sandbox it is asked about as usual");
        assert!(!matches!(plain(&command), CommandClass::Forbidden(_)), "{command}");
    }
    let why = sign_in_refused(&format!("{cmd} mcp login jira"), true).unwrap_or_default();
    assert!(why.contains("/mcp login jira"), "names the server: {why}");
    for command in [format!("{cmd} mcp list"), format!("{cmd} mcp add jira https://mcp.test/mcp"), "login".into()] {
        assert_eq!(sign_in_refused(&command, true), None, "{command}");
    }
}

#[test]
fn asking_what_a_program_is_or_how_to_use_it_only_reads() {
    let cmd = brand::COMMAND;
    for command in [
        "command -v github-mcp-server".to_owned(),
        "command -v github-mcp-server || true".to_owned(),
        "command -v gh docker".to_owned(),
        "grep --version".to_owned(),
        "ls --help".to_owned(),
        format!("{cmd} mcp list"),
        format!("{cmd} mcp add --help && {cmd} mcp remove --help"),
    ] {
        assert_eq!(plain(&command), CommandClass::ReadOnly, "{command}");
    }
    for command in [
        "./deploy.sh --help".to_owned(),
        "docker --version".to_owned(),
        "cargo --help".to_owned(),
        "/tmp/tool --version".to_owned(),
        "npx create-thing --version".to_owned(),
        "docker run --help".to_owned(),
        "command -v $(rm -rf x)".to_owned(),
        "command rm -rf build".to_owned(),
        format!("{cmd} mcp add jira https://mcp.test/mcp"),
        format!("{cmd} mcp list > out.txt"),
    ] {
        assert_ne!(plain(&command), CommandClass::ReadOnly, "{command}");
    }
}
