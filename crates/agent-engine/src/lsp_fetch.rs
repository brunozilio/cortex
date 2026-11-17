use std::io::Read;

use crate::model::CancellationToken;
use crate::model::http::{Http, NetHttp};

#[derive(Debug, Default, Clone, Copy)]
pub struct NetFetch;

impl language_servers::Fetch for NetFetch {
    fn get(&self, url: &str) -> Result<(u16, Box<dyn Read + Send>), String> {
        let never = CancellationToken::new();
        let response = platform::runtime::block_on(NetHttp.get_stream(url, &[], &never))
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        Ok((response.status, Box::new(response.body)))
    }
}
