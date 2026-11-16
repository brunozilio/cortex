use serde_json::Value;

pub trait Backend: Send + Sync {
    fn feature(&self, name: &str, input: Value) -> Result<Value, String>;
}

impl Backend for sdk::Sdk {
    fn feature(&self, name: &str, input: Value) -> Result<Value, String> {
        sdk::Sdk::feature(self, name, input).map_err(|error| error.to_string())
    }
}
