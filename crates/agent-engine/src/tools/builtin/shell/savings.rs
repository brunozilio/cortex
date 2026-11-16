use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use domain::output::Saving;
use platform::sync::Locked;

fn store() -> &'static Mutex<HashMap<u64, Vec<Saving>>> {
    static SAVINGS: OnceLock<Mutex<HashMap<u64, Vec<Saving>>>> = OnceLock::new();
    SAVINGS.get_or_init(Mutex::default)
}

pub fn record(client: u64, saving: Saving) {
    store().locked().entry(client).or_default().push(saving);
}

pub fn list(client: u64) -> Vec<Saving> {
    store().locked().get(&client).cloned().unwrap_or_default()
}

pub fn forget(client: u64) {
    store().locked().remove(&client);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_client_sees_only_its_own_savings_until_it_leaves() {
        let saving = Saving { program: "cargo".into(), before: 10, after: 4 };
        record(91_001, saving.clone());
        record(91_002, Saving { program: "git".into(), before: 5, after: 5 });
        assert_eq!(list(91_001), [saving]);
        forget(91_001);
        forget(91_002);
        assert!(list(91_001).is_empty() && list(91_002).is_empty());
    }
}
