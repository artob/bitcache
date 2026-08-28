// This is free and unencumbered software released into the public domain.

use alloc::{boxed::Box, string::String, vec, vec::Vec};
use bitcache_core::{
    Blob, BlobMetadata, Bytes, Id, ListOptions, ListOrder, Repository, RepositoryError, Stream,
    futures_util::{StreamExt, stream},
};
use reqwest::{Client, RequestBuilder, Url, header};
use serde::Deserialize;

const GITHUB_API: &str = "https://api.github.com";
const GITHUB_API_VERSION: &str = "2022-11-28";
const GITHUB_JSON: &str = "application/vnd.github+json";
const GITHUB_RAW: &str = "application/vnd.github.raw+json";
const USER_AGENT: &str = concat!("bitcache-git/", env!("CARGO_PKG_VERSION"));
const BITCACHE_DIR: &str = ".bitcache";

/// A read-only Bitcache repository stored in a Git repository.
///
/// Currently only public GitHub repositories are supported. Bitcache blobs are
/// read from the repository's `.bitcache/` directory and must use the flat
/// hexadecimal filename layout of `bitcache-fs`.
#[derive(Clone, Debug)]
pub enum GitRepository {
    /// A public repository hosted on GitHub.
    GitHub {
        owner: String,
        repo: String,
        branch: String,
    },
}

#[derive(Clone, Debug)]
struct GitHubBlob {
    id: Id,
    sha: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GitHubTree {
    tree: Vec<GitHubTreeEntry>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    sha: String,
    size: Option<u64>,
}

impl GitRepository {
    /// Creates a read-only repository backed by a public GitHub repository.
    pub fn github(
        owner: impl Into<String>,
        repo: impl Into<String>,
        branch: impl Into<String>,
    ) -> Self {
        Self::GitHub {
            owner: owner.into(),
            repo: repo.into(),
            branch: branch.into(),
        }
    }

    fn github_parts(&self) -> (&str, &str, &str) {
        match self {
            Self::GitHub {
                owner,
                repo,
                branch,
            } => (owner, repo, branch),
        }
    }

    fn github_url(&self, resource: &str, reference: &str) -> Url {
        let (owner, repo, _) = self.github_parts();
        let mut url = Url::parse(GITHUB_API).expect("the GitHub API URL is valid");
        url.path_segments_mut()
            .expect("the GitHub API URL can contain path segments")
            .extend(["repos", owner, repo, "git", resource, reference]);
        url
    }

    fn github_request(&self, url: Url, accept: &'static str) -> RequestBuilder {
        github_client()
            .get(url)
            .header(header::ACCEPT, accept)
            .header(header::USER_AGENT, USER_AGENT)
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION)
    }

    async fn github_tree(
        &self,
        reference: &str,
        recursive: bool,
    ) -> Result<GitHubTree, RepositoryError> {
        let mut url = self.github_url("trees", reference);
        if recursive {
            url.query_pairs_mut().append_pair("recursive", "1");
        }

        let tree = self
            .github_request(url, GITHUB_JSON)
            .send()
            .await
            .map_err(other_error)?
            .error_for_status()
            .map_err(other_error)?
            .json::<GitHubTree>()
            .await
            .map_err(other_error)?;
        if tree.truncated {
            return Err(other_message(
                "GitHub returned a truncated tree; the Bitcache repository cannot be enumerated completely",
            ));
        }
        Ok(tree)
    }

    async fn github_bitcache_tree(&self) -> Result<Option<GitHubTree>, RepositoryError> {
        let (_, _, branch) = self.github_parts();
        let root = self.github_tree(branch, false).await?;
        let Some(bitcache) = root
            .tree
            .into_iter()
            .find(|entry| entry.kind == "tree" && entry.path == BITCACHE_DIR)
        else {
            return Ok(None);
        };
        self.github_tree(&bitcache.sha, true).await.map(Some)
    }

    async fn collect_blobs(
        &self,
        options: &ListOptions,
    ) -> Result<Vec<GitHubBlob>, RepositoryError> {
        let Some(tree) = self.github_bitcache_tree().await? else {
            return Ok(Vec::new());
        };
        let mut blobs = tree
            .tree
            .into_iter()
            .filter_map(|entry| {
                if entry.kind != "blob" {
                    return None;
                }
                if entry.path.contains('/') {
                    return None;
                }
                let id = Id::from_hex(&entry.path).ok()?;
                options.matches(&id).then_some(GitHubBlob {
                    id,
                    sha: entry.sha,
                    size: entry.size,
                })
            })
            .collect::<Vec<_>>();

        blobs.sort_unstable_by(|left, right| left.id.cmp(&right.id));
        if options.order == Some(ListOrder::Descending) {
            blobs.reverse();
        }
        if let Some(limit) = options.limit {
            blobs.truncate(limit);
        }
        Ok(blobs)
    }

    async fn find_blob(&self, id: &Id) -> Result<Option<GitHubBlob>, RepositoryError> {
        Ok(self
            .collect_blobs(&ListOptions::new().with_prefix(id.to_hex().as_str()))
            .await?
            .into_iter()
            .find(|blob| &blob.id == id))
    }

    async fn fetch_blob(&self, blob: GitHubBlob) -> Result<Blob, RepositoryError> {
        let bytes = self
            .github_request(self.github_url("blobs", &blob.sha), GITHUB_RAW)
            .send()
            .await
            .map_err(other_error)?
            .error_for_status()
            .map_err(other_error)?
            .bytes()
            .await
            .map_err(other_error)?;
        let size = blob.size.unwrap_or(bytes.len() as u64);
        Ok(Blob::new_unchecked(blob.id, bytes).with_metadata(BlobMetadata::new(size)))
    }
}

impl Repository for GitRepository {
    type Error = RepositoryError;

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.find_blob(id).await?.is_some())
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        match self.find_blob(id).await? {
            Some(blob) => self.fetch_blob(blob).await.map(Some),
            None => Ok(None),
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        match self.find_blob(id).await? {
            Some(blob) => match blob.size {
                Some(size) => Ok(Some(size)),
                None => Ok(Some(self.fetch_blob(blob).await?.len())),
            },
            None => Ok(None),
        }
    }

    async fn put(&mut self, _data: Bytes) -> Result<Id, Self::Error> {
        Err(RepositoryError::UnsupportedOperation)
    }

    async fn remove(&mut self, _id: &Id) -> Result<bool, Self::Error> {
        Err(RepositoryError::UnsupportedOperation)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        Err(RepositoryError::UnsupportedOperation)
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        let repository = self.clone();
        stream::once(async move {
            match repository.collect_blobs(&options).await {
                Ok(blobs) => stream::iter(
                    blobs
                        .into_iter()
                        .map(|blob| Ok(blob.id))
                        .collect::<Vec<_>>(),
                ),
                Err(error) => stream::iter(vec![Err(error)]),
            }
        })
        .flatten()
        .boxed()
    }
}

fn github_client() -> &'static Client {
    static CLIENT: std::sync::LazyLock<Client> = std::sync::LazyLock::new(Client::new);
    &CLIENT
}

fn other_error(error: impl core::error::Error + Send + Sync + 'static) -> RepositoryError {
    RepositoryError::Other(Box::new(error))
}

fn other_message(message: &'static str) -> RepositoryError {
    other_error(std::io::Error::other(message))
}
