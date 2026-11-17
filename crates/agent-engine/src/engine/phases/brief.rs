use crate::brief::Brief;
use crate::engine::job::Job;
use crate::engine::stop::Stop;

impl Job {
    pub(in crate::engine) fn brief(&self) -> Result<Brief, Stop> {
        content::install_once();
        Brief::load(&self.request).map_err(Stop::failed)
    }
}
