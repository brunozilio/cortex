use std::collections::BTreeSet;
use std::path::Path;

const BASELINE: &str = "xtask/lint-baseline.txt";

pub fn run(root: &Path, update_baseline: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut found = BTreeSet::new();
    found.extend(crate::brand::findings(root)?);
    found.extend(crate::comments::findings(root)?);
    found.extend(crate::crate_table::findings(root)?);
    found.extend(crate::crate_graph::findings(root)?);
    found.extend(crate::drift::findings(root)?);
    found.extend(crate::env_vars::findings(root)?);
    found.extend(crate::git_commands::findings(root)?);
    let baseline_path = root.join(BASELINE);
    if update_baseline {
        std::fs::write(&baseline_path, found.iter().map(|f| format!("{f}\n")).collect::<String>())?;
        println!("baseline: {} findings", found.len());
        return Ok(());
    }
    let known: BTreeSet<String> =
        std::fs::read_to_string(&baseline_path).unwrap_or_default().lines().map(str::to_owned).collect();
    let new: Vec<_> = found.difference(&known).collect();
    for finding in &new {
        eprintln!("lint: {finding}");
    }
    if new.is_empty() {
        println!("lint: ok ({} baselined)", known.len());
        Ok(())
    } else {
        Err(format!("{} new lint finding(s)", new.len()).into())
    }
}
