use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::hosted;

#[test]
fn every_host_call_is_served_in_order_and_the_script_s_result_comes_back() {
    let mut served = Vec::new();
    let result = hosted(
        "crashed",
        |host| {
            let first = host.call("log", json!("one"))?;
            let second = host.call("llm", json!({"prompt": "two"}))?;
            Ok(json!([first, second]))
        },
        |function, arguments| {
            served.push(function.to_owned());
            Ok(json!({ "function": function, "arguments": arguments }))
        },
    );
    assert_eq!(served, ["log", "llm"]);
    assert_eq!(result.expect("result")[1]["arguments"]["prompt"], "two");
}

#[test]
fn a_finished_script_is_not_waited_on_even_if_its_host_is_still_held() {
    let kept: Arc<Mutex<Option<Arc<dyn script::Host>>>> = Arc::default();
    let keeper = Arc::clone(&kept);
    let started = Instant::now();
    let result = hosted(
        "crashed",
        move |host| {
            *keeper.lock().expect("kept") = Some(host);
            Ok(Value::Null)
        },
        |_, _| Ok(Value::Null),
    );
    assert_eq!(result, Ok(Value::Null));
    assert!(started.elapsed() < Duration::from_secs(5), "{:?}", started.elapsed());
    assert!(kept.lock().expect("kept").is_some());
}

#[test]
fn a_script_that_panics_is_reported_with_the_given_words() {
    let result = hosted("the hook script stopped unexpectedly", |_| panic!("boom"), |_, _| Ok(Value::Null));
    assert_eq!(result, Err("the hook script stopped unexpectedly".to_owned()));
}
