use serde_json::{Value, json};
use store::Health;

use crate::feature::Context;

pub fn handle(context: &Context<'_>, _input: Value) -> Result<Value, String> {
    let health = context.services.database.health();
    let mut database = json!({ "status": health.label() });
    if let Health::Broken(reason) = &health {
        database["reason"] = json!(reason);
    }
    Ok(json!({ "name": brand::NAME, "version": env!("CARGO_PKG_VERSION"), "database": database }))
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use store::{Database, Health};

    use super::*;
    use crate::feature::testing;

    struct Fixed(Health);

    impl Database for Fixed {
        fn health(&self) -> Health {
            self.0.clone()
        }

        fn preference(&self, _key: &str) -> Result<Option<Value>, String> {
            Err("unused".into())
        }

        fn keep_preference(&self, _key: &str, _value: &Value) -> Result<(), String> {
            Err("unused".into())
        }
    }

    fn instance(health: Health) -> Value {
        let mut services = testing::services(None);
        services.database = Arc::new(Fixed(health));
        testing::with(1, &services, |context| handle(context, json!({}))).expect("instance")
    }

    #[test]
    fn the_instance_names_the_product_and_its_version() {
        let instance = instance(Health::Ready);
        assert_eq!(instance["name"], brand::NAME);
        assert!(!instance["version"].as_str().unwrap_or_default().is_empty());
    }

    #[test]
    fn the_instance_says_whether_its_database_is_ready_missing_or_broken() {
        assert_eq!(instance(Health::Ready)["database"], json!({ "status": "ready" }));
        assert_eq!(instance(Health::Missing)["database"], json!({ "status": "missing" }));
        assert_eq!(
            instance(Health::Broken("connection refused".into()))["database"],
            json!({ "status": "broken", "reason": "connection refused" })
        );
    }
}
