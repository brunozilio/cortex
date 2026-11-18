use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use platform::sync::Locked;

use domain::inbox::{self, Notice};

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Notice>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Vec<Notice>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).locked()
}

pub fn push(client: u64, notice: Notice) {
    inbox::keep(lock().entry(client).or_default(), notice);
}

pub fn take(client: u64) -> Vec<Notice> {
    lock().remove(&client).unwrap_or_default()
}

pub fn take_where(client: u64, wanted: impl Fn(&Notice) -> bool) -> Vec<Notice> {
    let mut all = lock();
    let Some(notices) = all.get_mut(&client) else { return Vec::new() };
    let (taken, left): (Vec<Notice>, Vec<Notice>) =
        std::mem::take(notices).into_iter().partition(|notice| wanted(notice));
    *notices = left;
    taken
}

pub fn withdraw(client: u64, source: &str) {
    if let Some(notices) = lock().get_mut(&client) {
        notices.retain(|notice| notice.source != source);
    }
}

pub fn forget_client(client: u64) {
    lock().remove(&client);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_takes_its_own_notices_once() {
        push(8801, Notice::new("monitor bash_1", "ready"));
        push(8802, Notice::new("monitor bash_2", "theirs"));
        assert_eq!(take(8801), [Notice::new("monitor bash_1", "ready")]);
        assert!(take(8801).is_empty());
        forget_client(8802);
        assert!(take(8802).is_empty());
    }

    #[test]
    fn some_notices_can_be_taken_and_the_rest_wait() {
        push(8804, Notice::new("observer critic", "careful"));
        push(8804, Notice::new("monitor bash_1", "up"));
        let taken = take_where(8804, |notice| notice.source.starts_with("observer "));
        assert_eq!(taken, [Notice::new("observer critic", "careful")]);
        assert_eq!(take(8804), [Notice::new("monitor bash_1", "up")]);
    }

    #[test]
    fn a_notice_not_yet_taken_can_be_withdrawn_by_its_source() {
        push(8803, Notice::new("background bash_7", "`go test .` finished (exit code 1)."));
        push(8803, Notice::new("background bash_8", "`go test .` finished (exit code 0)."));
        withdraw(8803, "background bash_7");
        assert_eq!(take(8803), [Notice::new("background bash_8", "`go test .` finished (exit code 0).")]);
    }
}
