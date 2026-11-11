use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use domain::inbox::{self, Notice};

fn lock() -> std::sync::MutexGuard<'static, HashMap<u64, Vec<Notice>>> {
    static ALL: OnceLock<Mutex<HashMap<u64, Vec<Notice>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn push(client: u64, notice: Notice) {
    inbox::keep(lock().entry(client).or_default(), notice);
}

pub fn take(client: u64) -> Vec<Notice> {
    lock().remove(&client).unwrap_or_default()
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
}
