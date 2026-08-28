// This is free and unencumbered software released into the public domain.

use crate::OpenError;
use bitcache_core::{DynRepository, RepositoryError};

#[cfg(feature = "std")]
pub async fn open_env(
    name: impl AsRef<str>,
    default_value: impl AsRef<str>,
) -> Result<alloc::boxed::Box<DynRepository<'static, RepositoryError>>, OpenError> {
    use std::env::VarError;
    match std::env::var(name.as_ref()) {
        Ok(url) => open(url).await,
        Err(VarError::NotPresent) => open(default_value.as_ref()).await,
        Err(VarError::NotUnicode(_)) => Err(OpenError::InvalidUrl),
    }
}

/// Opens a Bitcache repository based on the given URL.
///
/// Public GitHub repositories can be opened using a Git URL such as
/// `git://github.com/asimov-datasets/gutenberg.org.git`. Git repositories are
/// read from their default branch.
pub async fn open(
    url: impl AsRef<str>,
) -> Result<alloc::boxed::Box<DynRepository<'static, RepositoryError>>, OpenError> {
    let url = url.as_ref();

    #[cfg(feature = "heap")]
    if url.is_empty() {
        return Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new()));
    }

    // Bare filesystem paths (not URLs):
    #[cfg(feature = "fs")]
    if url.starts_with('.') || url.starts_with('/') || !url.contains(':') {
        return Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            url,
        )?));
    }

    let parsed = url::Url::parse(url).map_err(|_| OpenError::InvalidUrl)?;
    match parsed.scheme() {
        #[cfg(feature = "git")]
        "git" => Ok(DynRepository::new_box(git_repository(&parsed)?)),

        #[cfg(feature = "heap")]
        "heap" | "memory" => Ok(DynRepository::new_box(bitcache_heap::HeapRepository::new())),

        // Note: dispatch on the parsed scheme, but pass down the original
        // path (rather than the parsed URL's, which WHATWG normalization
        // would have made absolute), so that relative paths keep working:
        #[cfg(feature = "fs")]
        "file" => Ok(DynRepository::new_box(bitcache_fs::FsRepository::open(
            url.strip_prefix("file:").unwrap(),
        )?)),

        #[cfg(feature = "opendal")]
        scheme if scheme.starts_with("opendal+") => Ok(DynRepository::new_box(
            bitcache_opendal::DalRepository::open(url.strip_prefix("opendal+").unwrap())?,
        )),

        #[cfg(any(feature = "valkey", feature = "redis"))]
        "valkey" | "valkeys" | "redis" | "rediss" => Ok(DynRepository::new_box(
            bitcache_valkey::ValkeyRepository::open(url)?,
        )),

        #[cfg(any(feature = "turso", feature = "sqlite"))]
        "sqlite" => Ok(DynRepository::new_box(
            bitcache_turso::TursoRepository::open(url).await.unwrap(),
        )),
        _ => Err(OpenError::UnknownAdapter),
    }
}

#[cfg(feature = "git")]
fn git_repository(parsed: &url::Url) -> Result<bitcache_git::GitRepository, OpenError> {
    if parsed.host_str() != Some("github.com") {
        return Err(OpenError::UnknownAdapter);
    }
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(OpenError::InvalidUrl);
    }

    let mut segments = parsed.path_segments().ok_or(OpenError::InvalidUrl)?;
    let owner = segments.next().filter(|segment| !segment.is_empty());
    let repo = segments.next().filter(|segment| !segment.is_empty());
    if segments.next().is_some() {
        return Err(OpenError::InvalidUrl);
    }
    let (Some(owner), Some(repo)) = (owner, repo) else {
        return Err(OpenError::InvalidUrl);
    };
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if repo.is_empty() {
        return Err(OpenError::InvalidUrl);
    }

    Ok(bitcache_git::GitRepository::github(owner, repo, "HEAD"))
}

#[cfg(all(test, feature = "git"))]
mod tests {
    use super::*;
    use bitcache_git::GitRepository;

    #[test]
    fn parses_github_git_urls() {
        for input in [
            "git://github.com/asimov-datasets/gutenberg.org.git",
            "git://github.com/asimov-datasets/gutenberg.org",
        ] {
            let parsed = url::Url::parse(input).unwrap();
            let repository = git_repository(&parsed).unwrap();
            assert!(matches!(
                repository,
                GitRepository::GitHub {
                    owner,
                    repo,
                    branch,
                } if owner == "asimov-datasets" && repo == "gutenberg.org" && branch == "HEAD"
            ));
        }
    }

    #[test]
    fn rejects_unsupported_or_malformed_git_urls() {
        let unsupported = url::Url::parse("git://gitlab.com/owner/repo.git").unwrap();
        assert!(matches!(
            git_repository(&unsupported),
            Err(OpenError::UnknownAdapter)
        ));

        for input in [
            "git://github.com/owner",
            "git://github.com/owner/repo/extra",
            "git://github.com/owner/repo?branch=master",
        ] {
            let parsed = url::Url::parse(input).unwrap();
            assert!(matches!(
                git_repository(&parsed),
                Err(OpenError::InvalidUrl)
            ));
        }
    }
}
