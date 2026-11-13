use std::io::Read;
use std::sync::atomic::AtomicBool;

use crate::model::http::{Http, NetHttp};

#[derive(Debug, Default, Clone, Copy)]
pub struct NetFetch;

impl language_servers::Fetch for NetFetch {
    fn get(&self, url: &str) -> Result<(u16, Box<dyn Read + Send>), String> {
        let never = AtomicBool::new(false);
        let response = NetHttp.get_stream(url, &[], &never).map_err(|error| error.to_string())?;
        Ok((response.status, Box::new(response.body)))
    }
}
