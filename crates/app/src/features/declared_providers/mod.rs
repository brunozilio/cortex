use crate::api::features::declared_providers::{DeclaredProvidersReply, DeclaredProvidersRequest};

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: DeclaredProvidersRequest) -> Result<DeclaredProvidersReply, String> {
    let lines = crate::engine::declare_providers(&context.services.gate(&request.path));
    Ok(DeclaredProvidersReply { lines })
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_providers_names_none_of_its_files() {
        let empty = tempfile::tempdir().expect("dir");
        crate::features::context::testing::local(1, |context| {
            let request = DeclaredProvidersRequest { path: empty.path().to_path_buf() };
            let listed = handle(context, request).expect("listed");
            assert!(listed.lines.iter().all(|line| !line.contains(&empty.path().display().to_string())));
        });
    }
}
