use serde_json::Value;

const TEMPLATE: &str = include_str!("trace.html");

pub fn page(session: &str, trace: &Value) -> String {
    let data = trace.to_string().replace('<', "\\u003c");
    TEMPLATE.replace("{{SESSION}}", &file_name(session)).replace("{{TRACE}}", &data)
}

pub fn file_name(session: &str) -> String {
    session.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn trace(text: &str) -> Value {
        json!({ "lane": { "steps": [{ "nodes": [{ "kind": "prompt", "text": text, "at_ms": 1, "until_ms": null, "failed": false }] }] },
                "totals": { "input": 0, "output": 0, "cache_read": 0, "cache_write": 0, "cost_usd": 0.0, "started_ms": 1, "ended_ms": 1 } })
    }

    fn embedded(page: &str) -> Value {
        let start = page.find("id=\"trace\">").expect("data") + "id=\"trace\">".len();
        let end = start + page[start..].find("</script>").expect("end");
        serde_json::from_str(&page[start..end]).expect("valid json")
    }

    #[test]
    fn the_page_carries_the_whole_trace_and_loads_nothing_from_the_network() {
        let page = page("s-1", &trace("debug my lambda"));
        assert_eq!(embedded(&page), trace("debug my lambda"));
        assert!(page.contains("s-1"));
        assert!(!page.contains("src=\"http") && !page.contains("href=\"http") && !page.contains("@import"));
    }

    #[test]
    fn content_that_looks_like_markup_cannot_break_out_of_the_page() {
        let hostile = "</script><script>alert(1)</script><!--";
        let page = page("<b>", &trace(hostile));
        assert_eq!(embedded(&page), trace(hostile), "the text survives intact");
        assert_eq!(page.matches("</script>").count(), TEMPLATE.matches("</script>").count());
        assert!(!page.contains("<b>"));
    }

    #[test]
    fn a_session_id_becomes_a_safe_file_name() {
        assert_eq!(file_name("ab-12/../x"), "ab-12____x");
    }
}
