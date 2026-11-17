use crate::brief::Brief;
use crate::engine::job::Job;
use crate::engine::stop::Stop;

impl Job {
    pub(in crate::engine) fn brief(&mut self) -> Result<Brief, Stop> {
        content::install_once();
        self.live.brief(&self.request).map_err(Stop::failed)
    }
}
