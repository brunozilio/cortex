use std::path::Path;
use std::sync::Arc;

use domain::trust::Trust;

use super::*;

fn repository(inside: &Path, name: &str) -> PathBuf {
    let repo = inside.join(name);
    std::fs::create_dir_all(repo.join(".git")).expect("repository");
    repo
}

#[test]
fn trusted_folders_are_kept_as_a_compact_list_under_the_user_s_data_and_read_by_another_instance() {
    let (data, work) = (tempfile::tempdir().expect("data"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    let trusts = Trusts::kept(Arc::new(FileTrustStore::new(data.path().join("users"))));

    assert_eq!(trusts.of(&repo, None), Trust::Untrusted);
    trusts.set(&repo.join("src"), None, true).expect("trusted");

    let file = data.path().join("users/local").join(FILE);
    let kept = std::fs::read_to_string(&file).expect("kept");
    let root = agent_engine::trust::root_of(&repo, None);
    assert!(kept.starts_with(&format!("[{{\"path\":{},\"trusted_at\":", serde_json::json!(root))), "{kept}");
    assert_eq!(kept.trim_end().lines().count(), 1, "compact: {kept}");
    let reopened = Trusts::kept(Arc::new(FileTrustStore::new(data.path().join("users"))));
    assert_eq!(reopened.of(&repo, None), Trust::Trusted);
    assert_eq!(reopened.of(&repo.join("src/bin"), None), Trust::Trusted, "any folder of the repository");

    reopened.set(&repo, None, false).expect("untrusted");
    assert_eq!(trusts.of(&repo, None), Trust::Untrusted, "taken back for every instance");
}

#[test]
fn trust_of_a_repository_does_not_reach_a_nested_repository_or_its_parent() {
    let work = tempfile::tempdir().expect("work");
    let repo = repository(work.path(), "rocket");
    let nested = repository(&repo, "vendor/engine");
    let trusts = Trusts::default();

    trusts.set(&repo, None, true).expect("trusted");

    assert_eq!(trusts.of(&nested, None), Trust::Untrusted);
    assert_eq!(trusts.of(work.path(), None), Trust::Untrusted);
    assert_eq!(trusts.report(&nested, None).root, agent_engine::trust::root_of(&nested, None));
}

#[cfg(unix)]
#[test]
fn a_folder_reached_through_a_link_is_the_folder_it_points_at() {
    let work = tempfile::tempdir().expect("work");
    let repo = repository(work.path(), "rocket");
    let link = work.path().join("shortcut");
    std::os::unix::fs::symlink(&repo, &link).expect("link");
    let trusts = Trusts::default();

    trusts.set(&link, None, true).expect("trusted through the link");

    assert_eq!(trusts.of(&repo, None), Trust::Trusted);
    assert_eq!(trusts.stored().listed().len(), 1);
    assert_eq!(trusts.stored().listed()[0].path, agent_engine::trust::root_of(&repo, None));
}

#[test]
fn trust_for_this_process_is_gone_with_the_process_and_untrusting_ends_it() {
    let work = tempfile::tempdir().expect("work");
    let repo = repository(work.path(), "rocket");
    let store = Arc::new(InMemoryTrustStore::default());
    let trusts = Trusts::kept(Arc::clone(&store) as Arc<dyn TrustStore>);

    trusts.only_for_this_process(&repo, None).expect("once");
    assert_eq!(trusts.of(&repo, None), Trust::Trusted);
    assert!(store.folders().listed().is_empty());
    assert_eq!(Trusts::kept(store as Arc<dyn TrustStore>).of(&repo, None), Trust::Untrusted, "another process");

    trusts.set(&repo, None, false).expect("untrust");
    assert_eq!(trusts.of(&repo, None), Trust::Untrusted);
}

#[test]
fn a_file_that_does_not_parse_trusts_nothing() {
    let (data, work) = (tempfile::tempdir().expect("data"), tempfile::tempdir().expect("work"));
    let file = data.path().join("users/local").join(FILE);
    std::fs::create_dir_all(file.parent().expect("folder")).expect("dir");
    std::fs::write(&file, "[{\"path\":").expect("broken");
    let trusts = Trusts::kept(Arc::new(FileTrustStore::new(data.path().join("users"))));

    assert_eq!(trusts.of(work.path(), None), Trust::Untrusted);
}

#[test]
fn the_home_folder_is_refused_by_kind_with_the_text_it_always_had() {
    let home = tempfile::tempdir().expect("home");
    let trusts = Trusts::default();
    let kept = trusts.set(home.path(), Some(home.path()), true).expect_err("home");
    let once = trusts.only_for_this_process(home.path(), Some(home.path())).expect_err("home");
    assert_eq!(kept, TrustError::Refused(Refusal::Home));
    assert_eq!(once, TrustError::Refused(Refusal::Home));
    assert_eq!(String::from(kept), Refusal::Home.to_string());
}
