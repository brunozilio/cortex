use std::path::PathBuf;

pub fn app_data_dir() -> Option<PathBuf> {
    platform::data_dir(brand::NAME, brand::COMMAND)
}

pub fn local_data_dir() -> Option<PathBuf> {
    platform::local_data_dir(brand::NAME, brand::COMMAND)
}

pub fn project_key(project: &std::path::Path) -> String {
    project.to_string_lossy().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect()
}

pub fn owner_key(owner: &domain::Principal) -> String {
    match owner {
        domain::Principal::LocalOwner => "local".to_owned(),
        domain::Principal::Member(member) => format!("org-{}-user-{}", member.org, member.user),
    }
}
