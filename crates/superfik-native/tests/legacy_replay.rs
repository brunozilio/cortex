//! Opt-in compatibility check against the owner's historical project files.
//! Logs only counts and parser diagnostics, never transcript text.
#[test]
#[ignore = "reads explicitly selected real project histories; set SUPERFIK_LEGACY_AUDIT_DIR"]
fn historical_project_transcripts_import_without_running_a_cli() {
    let root = std::env::var_os("SUPERFIK_LEGACY_AUDIT_DIR").expect("select project history directory");
    let mut count = 0;
    let mut messages = 0;
    for entry in std::fs::read_dir(root).expect("project histories") {
        let entry = entry.expect("entry");
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        if entry.metadata().expect("metadata").modified().expect("mtime").elapsed().is_ok_and(|d| d.as_secs() < 60) {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("transcript snapshot");
        let parsed = superfik_native::sessions::import_jsonl(&source)
            .unwrap_or_else(|error| panic!("historical fixture {} could not be imported: {error}", count + 1));
        count += 1;
        messages += parsed.len();
    }
    assert!(count > 0, "no inactive histories selected");
    println!("historical_transcripts_imported={count}; messages={messages}; cli_processes_started=0");
}
