//! `owner/name` slugs from git remote URLs. Pure.

pub fn remote_slug(url: &str) -> Option<String> {
    let path = remote_path(url.trim())?;
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    let (name, owner) = match segments.as_slice() {
        [.., owner, name] => (*name, Some(*owner)),
        [name] => (*name, None),
        [] => return None,
    };
    let name = name.strip_suffix(".git").unwrap_or(name);
    if name.is_empty() || segments.iter().any(|segment| segment.contains(char::is_whitespace)) {
        return None;
    }
    Some(match owner {
        Some(owner) => format!("{owner}/{name}"),
        None => name.to_owned(),
    })
}

fn remote_path(url: &str) -> Option<&str> {
    if url.is_empty() {
        return None;
    }
    if let Some((_, rest)) = url.split_once("://") {
        return rest.split_once('/').map(|(_, path)| path);
    }
    match url.split_once(':') {
        Some((host, path)) if !host.contains('/') && host.len() > 1 => Some(path),
        _ => Some(url),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_urls() {
        assert_eq!(
            remote_slug("https://github.com/zzzzshawn/dither-minis.git").as_deref(),
            Some("zzzzshawn/dither-minis")
        );
        assert_eq!(remote_slug("https://github.com/owner/name").as_deref(), Some("owner/name"));
        assert_eq!(remote_slug("https://github.com/owner/name/").as_deref(), Some("owner/name"));
        assert_eq!(remote_slug("https://user:token@github.com/owner/name.git").as_deref(), Some("owner/name"));
    }

    #[test]
    fn ssh_urls() {
        assert_eq!(remote_slug("git@github.com:owner/name.git").as_deref(), Some("owner/name"));
        assert_eq!(remote_slug("ssh://git@github.com:22/owner/name.git").as_deref(), Some("owner/name"));
        assert_eq!(remote_slug("  git@gitlab.com:group/sub/name.git\n").as_deref(), Some("sub/name"));
    }

    #[test]
    fn local_and_degenerate_urls() {
        assert_eq!(remote_slug("/srv/git/name.git").as_deref(), Some("git/name"));
        assert_eq!(remote_slug("name.git").as_deref(), Some("name"));
        assert_eq!(remote_slug(""), None);
        assert_eq!(remote_slug("https://github.com"), None);
        assert_eq!(remote_slug("https://github.com/.git"), None);
        assert_eq!(remote_slug("not a url"), None);
    }
}
