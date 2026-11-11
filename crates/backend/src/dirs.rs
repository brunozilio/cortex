use std::path::PathBuf;

pub fn app_data_dir() -> Option<PathBuf> {
    platform::data_dir(brand::NAME, brand::COMMAND)
}

pub fn local_data_dir() -> Option<PathBuf> {
    platform::local_data_dir(brand::NAME, brand::COMMAND)
}
