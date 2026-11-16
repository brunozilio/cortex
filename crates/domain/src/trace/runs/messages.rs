use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::trace::ModelCall;

const IMAGE_SHOWN: usize = 256;

#[derive(Debug, Default)]
pub(super) struct Messages {
    systems: HashMap<String, String>,
    tool_sets: HashMap<String, Value>,
    pool: Vec<Value>,
    threads: HashMap<u32, Vec<u32>>,
    used_systems: BTreeMap<String, String>,
    used_tools: BTreeMap<String, Value>,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Sent {
    pub request: Option<Vec<u32>>,
    pub reply: Option<Value>,
    pub system: Option<String>,
    pub tools: Option<String>,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Kept {
    pub messages: Vec<Value>,
    pub systems: BTreeMap<String, String>,
    pub tools: BTreeMap<String, Value>,
}

impl Messages {
    pub(super) fn begin(&mut self) {
        self.pool.clear();
        self.threads.clear();
        self.used_systems.clear();
        self.used_tools.clear();
    }

    pub(super) fn sent(&mut self, thread: u32, call: &ModelCall) -> Sent {
        if let Some(system) = &call.system {
            self.systems.insert(call.system_id.clone(), system.clone());
        }
        if let Some(tools) = &call.tools {
            self.tool_sets.insert(call.tools_id.clone(), tools.clone());
        }
        let system = self.systems.get(&call.system_id).map(|text| {
            self.used_systems.insert(call.system_id.clone(), text.clone());
            call.system_id.clone()
        });
        let tools = self.tool_sets.get(&call.tools_id).map(|set| {
            self.used_tools.insert(call.tools_id.clone(), set.clone());
            call.tools_id.clone()
        });
        let request = call.added.as_ref().map(|added| {
            let pool = &mut self.pool;
            let list = self.threads.entry(thread).or_default();
            list.truncate(call.keep);
            for message in added {
                pool.push(lightened(message));
                list.push(u32::try_from(pool.len() - 1).unwrap_or(u32::MAX));
            }
            list.clone()
        });
        Sent { request, reply: call.reply.as_ref().map(lightened), system, tools }
    }

    pub(super) fn finish(&mut self) -> Kept {
        Kept {
            messages: std::mem::take(&mut self.pool),
            systems: std::mem::take(&mut self.used_systems),
            tools: std::mem::take(&mut self.used_tools),
        }
    }
}

pub(super) fn lightened(value: &Value) -> Value {
    match value {
        Value::Object(fields) => {
            let image = fields.get("media_type").and_then(Value::as_str).filter(|kind| kind.starts_with("image/"));
            let data = fields.get("data").and_then(Value::as_str);
            if let (Some(kind), Some(data)) = (image, data)
                && data.len() > IMAGE_SHOWN
            {
                let mut kept = fields.clone();
                kept.insert("data".into(), Value::String(format!("[{kind}, {} KB]", data.len() * 3 / 4 / 1024)));
                return Value::Object(kept);
            }
            Value::Object(fields.iter().map(|(key, value)| (key.clone(), lightened(value))).collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(lightened).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn call(keep: usize, added: Option<Vec<Value>>, system: Option<&str>) -> ModelCall {
        ModelCall {
            model: "m".into(),
            system_id: "sys".into(),
            system: system.map(str::to_owned),
            tools_id: "tools".into(),
            tools: system.map(|_| json!([{ "name": "Read" }])),
            keep,
            added,
            ..ModelCall::default()
        }
    }

    #[test]
    fn each_call_sees_the_messages_kept_from_the_last_one_plus_the_new_ones() {
        let mut messages = Messages::default();
        messages.begin();
        let first = messages.sent(1, &call(0, Some(vec![json!("u1")]), Some("be brief")));
        let second = messages.sent(1, &call(1, Some(vec![json!("a1"), json!("u2")]), None));
        let rewritten = messages.sent(1, &call(1, Some(vec![json!("summary")]), None));

        assert_eq!(first.request, Some(vec![0]));
        assert_eq!(second.request, Some(vec![0, 1, 2]));
        assert_eq!(rewritten.request, Some(vec![0, 3]), "a rewrite keeps only the shared start");
        assert_eq!((second.system.as_deref(), second.tools.as_deref()), (Some("sys"), Some("tools")));

        let kept = messages.finish();
        assert_eq!(kept.messages, [json!("u1"), json!("a1"), json!("u2"), json!("summary")]);
        assert_eq!(kept.systems.get("sys").map(String::as_str), Some("be brief"));
        assert_eq!(kept.tools.get("tools"), Some(&json!([{ "name": "Read" }])));
    }

    #[test]
    fn each_agent_keeps_its_own_conversation() {
        let mut messages = Messages::default();
        messages.sent(1, &call(0, Some(vec![json!("main")]), Some("s")));
        let agent = messages.sent(7, &call(0, Some(vec![json!("task")]), None));
        let main = messages.sent(1, &call(1, Some(vec![json!("more")]), None));
        assert_eq!(agent.request, Some(vec![1]));
        assert_eq!(main.request, Some(vec![0, 2]));
    }

    #[test]
    fn a_call_logged_before_messages_were_kept_has_no_request_but_still_its_system() {
        let mut messages = Messages::default();
        let old = messages.sent(1, &call(0, None, Some("the rules")));
        assert_eq!(old.request, None);
        assert_eq!(old.system.as_deref(), Some("sys"));
    }

    #[test]
    fn a_system_prompt_from_an_earlier_run_is_still_shown_in_a_later_one() {
        let mut messages = Messages::default();
        messages.begin();
        messages.sent(1, &call(0, None, Some("the rules")));
        messages.finish();
        messages.begin();
        messages.sent(1, &call(0, None, None));
        assert_eq!(messages.finish().systems.get("sys").map(String::as_str), Some("the rules"));
    }

    #[test]
    fn a_big_image_is_named_by_its_type_and_size_instead_of_its_bytes() {
        let data = "A".repeat(4096);
        let message =
            json!({ "blocks": [{ "Image": { "media_type": "image/png", "data": data } }, { "Text": "look" }] });
        let light = lightened(&message);
        assert_eq!(light["blocks"][0]["Image"]["data"], "[image/png, 3 KB]");
        assert_eq!(light["blocks"][1]["Text"], "look");
        let small = json!({ "media_type": "image/png", "data": "abc" });
        assert_eq!(lightened(&small), small);
    }
}
