// This is free and unencumbered software released into the public domain.

use crate::{
    BlobEncoding, Dir, DirCursor, Utf8Path,
    dir_cursor::{BLOBS_DIR, SHARD_PREFIX_LEN},
    file_metadata::{self, ExtendedMetadata},
};
#[cfg(feature = "tokio")]
use crate::BlobFile;
use bitcache_core::{
    Blob, BlobMetadata, BlobMetadataCapabilities, Bytes, CompactOptions, Compression, Id,
    ListOptions, ListOrder, PutOptions, Repository, RepositoryCapabilities, RepositoryError,
    Stream, futures_util::StreamExt,
};
#[cfg(unix)]
use cap_std::fs_utf8::{MetadataExt, PermissionsExt};
use cap_std::{
    ambient_authority,
    fs_utf8::{File, OpenOptions},
};
use std::{
    path::PathBuf,
    string::{String, ToString},
    sync::atomic::{AtomicU64, Ordering},
    vec::Vec,
};

/// The buffer size used when streaming file contents.
#[cfg(feature = "tokio")]
const BUFFER_LEN: usize = 65_536;

/// The LZMA2 dictionary size used by liblzma's fastest (preset 0) mode.
#[cfg(feature = "tokio")]
const FASTEST_XZ_DICTIONARY_SIZE: u64 = 256 * 1024;

/// A process-local sequence used to make temporary filenames unique.
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// The compression scheme used to store blobs when the caller leaves
/// [`PutOptions::compression`] unset.
const DEFAULT_PUT_COMPRESSION: Compression = Compression::XzFast;

/// The common prefix of all temporary artifacts at the repository root
/// (`.tmp-put-*` files, `.tmp-clear-*` directories), so that an admin can
/// locate and delete them all with a single `.tmp-*` wildcard.
const TEMP_PREFIX: &str = ".tmp-";

/// The minimum age at which a temporary file is considered orphaned.
const TEMP_ORPHAN_TTL: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// The name of the Git attributes file created at the repository root.
const GIT_ATTRIBUTES_NAME: &str = ".gitattributes";

/// The contents of the Git attributes file created at the repository root.
///
/// Marks all blob files as binary so that Git never attempts text
/// conversion or diffing on them. When Git LFS (or Hugging Face Xet)
/// support lands, this is where blobs will be routed through the
/// appropriate filter (e.g., `blobs/** filter=lfs diff=lfs merge=lfs -text`).
const GIT_ATTRIBUTES: &str = "blobs/** binary\n";

/// The name of the Git ignore file created at the repository root.
const GIT_IGNORE_NAME: &str = ".gitignore";

/// The contents of the Git ignore file created at the repository root:
/// temporary artifacts must never be committed.
const GIT_IGNORE: &str = ".tmp-*\n";

struct PhysicalBlob {
    name: String,
    file: File,
    extended: ExtendedMetadata,
    encoding: BlobEncoding,
}

/// Options for creating a repository with [`FsRepository::create_with_options`].
#[derive(Clone, Copy, Debug)]
pub struct CreateOptions {
    /// Whether to create the default Git metadata files (`.gitattributes`,
    /// `.gitignore`) at the repository root (the default).
    pub git: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self { git: true }
    }
}

impl CreateOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to create the default Git metadata files.
    pub fn with_git(mut self, git: bool) -> Self {
        self.git = git;
        self
    }
}

/// A repository backed by a local filesystem directory.
///
/// Blobs live under a dedicated `blobs` subdirectory, sharded into
/// subdirectories named after the first [`SHARD_PREFIX_LEN`] hexadecimal
/// characters of the blob ID (`00` through `ff` by default) and created
/// lazily as blobs are stored. Within a shard, files are named by their full
/// hexadecimal IDs, with an `.xz` suffix when compressed:
///
/// ```text
/// .bitcache/
/// ├── .gitattributes
/// ├── .gitignore
/// └── blobs/
///     └── ab/
///         └── abcdef….xz
/// ```
///
/// The repository root remains free for auxiliary files (configuration,
/// Git metadata, etc.), and clearing the repository is atomic: the `blobs`
/// directory is renamed aside and deleted offline.
///
/// Access is capability-scoped to the directory
/// via [`cap_std`]. On Unix, extended
/// metadata is also accessed through file handles. Windows extended metadata
/// uses NTFS alternate data streams and therefore retains the canonical
/// repository path as a platform-specific fallback.
///
/// Note: except for [`FsRepository::put_file`], I/O is currently performed
/// synchronously within the async methods.
#[derive(Debug)]
pub struct FsRepository(Dir, RepositoryCapabilities, #[cfg(windows)] PathBuf);

impl FsRepository {
    /// Creates or opens a new repository at the given directory path, with
    /// default [`CreateOptions`].
    ///
    /// Ensures that the `blobs` subdirectory exists and that default
    /// `.gitattributes` (marking blobs as binary) and `.gitignore` (ignoring
    /// temporary artifacts) files are present; existing files are never
    /// overwritten.
    pub fn create(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        Self::create_with_options(path, CreateOptions::default())
    }

    /// Creates or opens a new repository at the given directory path.
    ///
    /// Ensures that the `blobs` subdirectory exists. Unless
    /// [`CreateOptions::git`] is disabled, also ensures that default
    /// `.gitattributes` (marking blobs as binary) and `.gitignore` (ignoring
    /// temporary artifacts) files are present; existing files are never
    /// overwritten.
    pub fn create_with_options(
        path: impl AsRef<Utf8Path>,
        options: CreateOptions,
    ) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let dir = match Dir::open_ambient_dir(path, ambient_authority()) {
            Ok(dir) => dir,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Dir::create_ambient_dir_all(path, ambient_authority())?;
                Dir::open_ambient_dir(path, ambient_authority())?
            },
            Err(error) => return Err(error.into()),
        };
        dir.create_dir_all(BLOBS_DIR)?;
        if options.git {
            Self::create_file_if_absent(&dir, GIT_ATTRIBUTES_NAME, GIT_ATTRIBUTES)?;
            Self::create_file_if_absent(&dir, GIT_IGNORE_NAME, GIT_IGNORE)?;
        }
        Self::from_dir(path, dir)
    }

    /// Opens the repository at the given directory path.
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self, RepositoryError> {
        let path = path.as_ref();
        let dir = Dir::open_ambient_dir(path, ambient_authority())?;
        Self::from_dir(path, dir)
    }

    #[cfg(not(windows))]
    fn from_dir(_path: &Utf8Path, dir: Dir) -> Result<Self, RepositoryError> {
        let mut repository = Self(dir, RepositoryCapabilities::NONE);
        repository.1 = repository.detect_capabilities();
        Ok(repository)
    }

    #[cfg(windows)]
    fn from_dir(path: &Utf8Path, dir: Dir) -> Result<Self, RepositoryError> {
        let mut repository = Self(
            dir,
            RepositoryCapabilities::NONE,
            std::fs::canonicalize(path.as_std_path())?,
        );
        repository.1 = repository.detect_capabilities();
        Ok(repository)
    }

    /// Writes a repository metadata file if it doesn't already exist.
    fn create_file_if_absent(dir: &Dir, name: &str, contents: &str) -> Result<(), RepositoryError> {
        use std::io::Write;

        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        match dir.open_with(name, &options) {
            Ok(mut file) => Ok(file.write_all(contents.as_bytes())?),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Creates a cursor over the physical blobs in the repository directory.
    ///
    /// See [`DirCursor`] for ordering and memory-use guarantees.
    fn cursor(&self, descending: bool) -> DirCursor {
        DirCursor::open(&self.0, descending)
    }

    /// Creates the shard subdirectory for the given blob ID if it doesn't
    /// already exist.
    ///
    /// Shard subdirectories (and the `blobs` directory itself, e.g., after
    /// an atomic clear) are created lazily, on first store of a blob whose
    /// ID falls within the shard.
    fn ensure_shard_dir(&self, id: &Id) -> Result<(), RepositoryError> {
        let path = std::format!("{}/{}", BLOBS_DIR, Self::shard_name(id));
        Ok(self.0.create_dir_all(path)?)
    }

    /// The shard subdirectory name for the blob with the given ID.
    fn shard_name(id: &Id) -> String {
        let hex = id.to_hex();
        hex.as_str()[..SHARD_PREFIX_LEN].into()
    }

    /// The uncompressed storage path for the blob with the given ID.
    fn path(id: &Id) -> String {
        let hex = id.to_hex();
        let hex = hex.as_str();
        std::format!("{}/{}/{}", BLOBS_DIR, &hex[..SHARD_PREFIX_LEN], hex)
    }

    /// The compressed storage path for the blob with the given ID.
    fn compressed_path(id: &Id) -> String {
        let hex = id.to_hex();
        let hex = hex.as_str();
        std::format!("{}/{}/{}.xz", BLOBS_DIR, &hex[..SHARD_PREFIX_LEN], hex)
    }

    fn path_for(id: &Id, encoding: BlobEncoding) -> String {
        match encoding {
            BlobEncoding::Uncompressed => Self::path(id),
            BlobEncoding::Xz => Self::compressed_path(id),
        }
    }

    #[cfg(not(windows))]
    fn extended_path(&self, _name: &str) -> Option<PathBuf> {
        None
    }

    #[cfg(windows)]
    fn extended_path(&self, name: &str) -> Option<PathBuf> {
        Some(self.2.join(name))
    }

    fn detect_capabilities(&self) -> RepositoryCapabilities {
        let metadata = BlobMetadataCapabilities::new()
            .with_created()
            .with_accessed();
        #[cfg(unix)]
        let metadata = metadata.with_updated();
        let metadata = if self.supports_extended_metadata() {
            metadata.with_expires().with_media_type()
        } else {
            metadata
        };
        RepositoryCapabilities::new().with_blob_metadata(metadata)
    }

    fn supports_extended_metadata(&self) -> bool {
        let Ok((name, file)) = self.create_temp_file() else {
            return false;
        };
        let metadata = ExtendedMetadata {
            expires: Some(1),
            media_type: Some(String::from("application/x-bitcache-capability-probe")),
            ..ExtendedMetadata::default()
        };
        let path = self.extended_path(&name);
        let supported = file_metadata::write(&file, path.as_deref(), &metadata).unwrap_or(false);
        drop(file);
        let _ = self.0.remove_file(name);
        supported
    }

    /// Derives blob metadata from filesystem and extended metadata.
    fn blob_metadata(
        metadata: &cap_std::fs::Metadata,
        extended: ExtendedMetadata,
        len: u64,
    ) -> BlobMetadata {
        let created = extended.created.or_else(|| {
            metadata
                .created()
                .ok()
                .and_then(|time| Self::system_time_nanos(time.into_std()))
        });
        let updated = extended.updated.or_else(|| Self::updated_nanos(metadata));
        BlobMetadata::new(len)
            .with_media_type(extended.media_type.map(Into::into))
            .with_created_nanos(created)
            .with_updated_nanos(updated)
            .with_accessed(metadata.accessed().ok().map(|time| time.into_std()))
            .with_expires_nanos(extended.expires)
    }

    fn system_time_nanos(time: std::time::SystemTime) -> Option<u64> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()?
            .as_nanos()
            .try_into()
            .ok()
    }

    /// Returns the inode status-change time as nanoseconds since the Unix epoch.
    #[cfg(unix)]
    fn updated_nanos(metadata: &cap_std::fs::Metadata) -> Option<u64> {
        let seconds = u64::try_from(metadata.ctime()).ok()?;
        let nanoseconds = u64::try_from(metadata.ctime_nsec()).ok()?;
        if nanoseconds >= 1_000_000_000 {
            return None;
        }
        seconds.checked_mul(1_000_000_000)?.checked_add(nanoseconds)
    }

    /// Status-change time is unavailable on non-Unix platforms.
    #[cfg(not(unix))]
    fn updated_nanos(_metadata: &cap_std::fs::Metadata) -> Option<u64> {
        None
    }

    fn now_nanos() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| duration.as_nanos().try_into().ok())
            .unwrap_or(u64::MAX)
    }

    fn read_extended(&self, name: &str, file: &File) -> Result<ExtendedMetadata, RepositoryError> {
        let path = self.extended_path(name);
        Ok(file_metadata::read(file, path.as_deref())?)
    }

    fn write_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        let path = self.extended_path(name);
        Ok(file_metadata::write(file, path.as_deref(), metadata)?)
    }

    fn mutate_read_only_file<T>(
        file: &File,
        operation: impl FnOnce() -> Result<T, RepositoryError>,
    ) -> Result<T, RepositoryError> {
        let original = file.metadata()?.permissions();
        let mut writable = original.clone();
        #[cfg(unix)]
        writable.set_mode(original.mode() | 0o200);
        #[cfg(not(unix))]
        writable.set_readonly(false);
        file.set_permissions(writable)?;

        let result = operation();
        let restore = file
            .set_permissions(original)
            .map_err(RepositoryError::from);
        match (result, restore) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn write_existing_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        Self::mutate_read_only_file(file, || self.write_extended(name, file, metadata))
    }

    fn write_reinserted_extended(
        &self,
        name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<bool, RepositoryError> {
        let existing = self.read_extended(name, file)?;
        let metadata = ExtendedMetadata {
            created: existing.created,
            updated: None,
            expires: metadata.expires,
            media_type: metadata.media_type.clone(),
        };
        self.write_existing_extended(name, file, &metadata)
    }

    fn is_expired(metadata: &ExtendedMetadata) -> bool {
        metadata
            .expires
            .is_some_and(|expires| expires <= Self::now_nanos())
    }

    /// Opens a physical blob and reads its extended metadata.
    ///
    /// The uncompressed representation is preferred when both exist.
    fn open_physical(&self, id: &Id) -> Result<Option<PhysicalBlob>, RepositoryError> {
        for encoding in [BlobEncoding::Uncompressed, BlobEncoding::Xz] {
            let name = Self::path_for(id, encoding);
            match self.0.open(&name) {
                Ok(file) => {
                    let extended = self.read_extended(&name, &file)?;
                    return Ok(Some(PhysicalBlob {
                        name,
                        file,
                        extended,
                        encoding,
                    }));
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    /// Opens a blob only if it is logically present (not expired).
    fn open_live(&self, id: &Id) -> Result<Option<PhysicalBlob>, RepositoryError> {
        Ok(self
            .open_physical(id)?
            .filter(|blob| !Self::is_expired(&blob.extended)))
    }

    /// Creates a uniquely named temporary file without replacing any file.
    fn create_temp_file(&self) -> Result<(String, File), RepositoryError> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);

        loop {
            let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let name = std::format!("{}put-{}-{}", TEMP_PREFIX, std::process::id(), sequence);
            match self.0.open_with(&name, &options) {
                Ok(file) => return Ok((name, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Sets the file's permissions to read-only, updating its status-change time.
    fn make_read_only(file: &File) -> Result<(), RepositoryError> {
        let mut permissions = file.metadata()?.permissions();
        #[cfg(unix)]
        permissions.set_mode(0o444);
        #[cfg(not(unix))]
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
        Ok(())
    }

    fn prepare_temp(
        &self,
        temp_name: &str,
        file: &File,
        metadata: &ExtendedMetadata,
    ) -> Result<(), RepositoryError> {
        self.write_extended(temp_name, file, metadata)?;
        Self::make_read_only(file)
    }

    #[cfg(feature = "tokio")]
    fn prepare_compacted_temp(
        &self,
        temp_name: &str,
        file: &File,
        extended: &ExtendedMetadata,
        source: &cap_std::fs::Metadata,
    ) -> Result<(), RepositoryError> {
        if !self.write_extended(temp_name, file, extended)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "filesystem cannot preserve blob metadata during compaction",
            )
            .into());
        }

        let mut times = std::fs::FileTimes::new();
        if let Ok(accessed) = source.accessed() {
            times = times.set_accessed(accessed.into_std());
        }
        if let Ok(modified) = source.modified() {
            times = times.set_modified(modified.into_std());
        }
        file.try_clone()?.into_std().set_times(times)?;
        file.set_permissions(source.permissions())?;
        Ok(())
    }

    /// Publishes a temporary file without ever replacing an existing blob.
    fn publish_temp(
        &self,
        temp_name: &str,
        id: &Id,
        metadata: &ExtendedMetadata,
        encoding: BlobEncoding,
    ) -> Result<(), RepositoryError> {
        use std::io::{Error, ErrorKind};

        let uncompressed_path = Self::path(id);
        match self.0.open(&uncompressed_path) {
            Ok(existing_file) => {
                let result = self
                    .write_reinserted_extended(&uncompressed_path, &existing_file, metadata)
                    .map(|_| ());
                let _ = self.0.remove_file(temp_name);
                return result;
            },
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                return Err(error.into());
            },
        }

        let path = Self::path_for(id, encoding);
        if let Err(error) = self.ensure_shard_dir(id) {
            let _ = self.0.remove_file(temp_name);
            return Err(error);
        }
        for _ in 0..4 {
            match self.0.hard_link(temp_name, &self.0, &path) {
                Ok(()) => {
                    self.0.remove_file(temp_name)?;
                    return Ok(());
                },
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    match self.0.open(&path) {
                        Ok(existing_file) => {
                            let result = self
                                .write_reinserted_extended(&path, &existing_file, metadata)
                                .map(|_| ());
                            let _ = self.0.remove_file(temp_name);
                            return result;
                        },
                        Err(error) if error.kind() == ErrorKind::NotFound => continue,
                        Err(error) => {
                            let _ = self.0.remove_file(temp_name);
                            return Err(error.into());
                        },
                    }
                },
                Err(error) => {
                    let _ = self.0.remove_file(temp_name);
                    return Err(error.into());
                },
            }
        }
        let _ = self.0.remove_file(temp_name);
        Err(Error::new(
            ErrorKind::NotFound,
            "blob disappeared repeatedly while being stored",
        )
        .into())
    }

    fn store_metadata(expires: Option<u64>, media_type: Option<String>) -> ExtendedMetadata {
        ExtendedMetadata {
            expires,
            media_type,
            ..ExtendedMetadata::default()
        }
    }

    /// Selects a blob that needs recompression toward the given target.
    ///
    /// Uncompressed blobs are always candidates. Existing XZ blobs are only
    /// candidates when recompressing to `xz:best` and they were written with
    /// the fastest (smallest-dictionary) preset.
    #[cfg(feature = "tokio")]
    fn compact_source(
        &self,
        id: &Id,
        recompress_fast_xz: bool,
    ) -> Result<Option<(BlobEncoding, cap_std::fs::Metadata)>, RepositoryError> {
        use crate::util::read_xz_dict_size;
        use std::io::ErrorKind;

        match self.0.open(Self::path(id)) {
            Ok(file) => {
                let metadata = file.metadata()?;
                return Ok(Some((BlobEncoding::Uncompressed, metadata)));
            },
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }

        if !recompress_fast_xz {
            return Ok(None);
        }
        let mut file = match self.0.open(Self::compressed_path(id)) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let metadata = file.metadata()?;
        Ok(
            (read_xz_dict_size(&mut file)? == Some(FASTEST_XZ_DICTIONARY_SIZE))
                .then_some((BlobEncoding::Xz, metadata)),
        )
    }

    /// The XZ compression level for the given scheme, or `None` for
    /// uncompressed storage.
    #[cfg(feature = "tokio")]
    fn xz_level(compression: Compression) -> Option<async_compression::Level> {
        match compression {
            Compression::None => None,
            Compression::XzFast => Some(async_compression::Level::Fastest),
            Compression::XzBest => Some(async_compression::Level::Best),
        }
    }

    /// Rewrites a blob using the given target compression, if it needs it.
    #[cfg(feature = "tokio")]
    async fn compact_blob(
        &self,
        id: &Id,
        compression: Compression,
    ) -> Result<(), RepositoryError> {
        use async_compression::tokio::write::XzEncoder;
        use bitcache_core::tokio::{fs::File, io::AsyncWriteExt};

        let Some(level) = Self::xz_level(compression) else {
            // Target `none`: existing blob encodings are left as they are.
            return Ok(());
        };
        let recompress_fast_xz = compression == Compression::XzBest;
        let Some((source_encoding, source_metadata)) =
            self.compact_source(id, recompress_fast_xz)?
        else {
            return Ok(());
        };
        let source_name = Self::path_for(id, source_encoding);
        let source_file = self.0.open(&source_name)?;
        let mut extended = self.read_extended(&source_name, &source_file)?;
        extended.created = extended.created.or_else(|| {
            source_metadata
                .created()
                .ok()
                .and_then(|time| Self::system_time_nanos(time.into_std()))
        });
        extended.updated = extended
            .updated
            .or_else(|| Self::updated_nanos(&source_metadata));
        let mut source_file = BlobFile::new(source_file, source_encoding);
        let (temp_name, temp_file) = self.create_temp_file()?;
        let temp_file = File::from_std(temp_file.into_std());
        let mut encoder = XzEncoder::with_quality(temp_file, level);

        let result: Result<(), RepositoryError> = async {
            bitcache_core::tokio::io::copy(&mut source_file, &mut encoder).await?;
            encoder.shutdown().await?;
            Ok(())
        }
        .await;
        drop(encoder);
        drop(source_file);

        if let Err(error) = result {
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }

        let temp_file = match self.0.open(&temp_name) {
            Ok(file) => file,
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                return Err(error.into());
            },
        };
        if let Err(error) =
            self.prepare_compacted_temp(&temp_name, &temp_file, &extended, &source_metadata)
        {
            drop(temp_file);
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }
        drop(temp_file);

        let compressed_name = Self::compressed_path(id);
        if let Err(error) = self.ensure_shard_dir(id) {
            let _ = self.0.remove_file(&temp_name);
            return Err(error);
        }
        if let Err(error) = self.0.rename(&temp_name, &self.0, &compressed_name) {
            let _ = self.0.remove_file(&temp_name);
            return Err(error.into());
        }
        if source_encoding == BlobEncoding::Xz {
            return Ok(());
        }
        match self.0.remove_file(source_name) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Opens the blob with the given ID for asynchronous streaming reads.
    ///
    /// Returns `Ok(None)` if the repository doesn't contain the blob or it has
    /// expired. The returned file handle is capability-scoped to the repository
    /// directory, and its contents can be read incrementally without buffering
    /// the whole blob in memory.
    #[cfg(feature = "tokio")]
    pub async fn get_file(&self, id: &Id) -> Result<Option<BlobFile>, RepositoryError> {
        Ok(self
            .open_live(id)?
            .map(|blob| BlobFile::new(blob.file, blob.encoding)))
    }

    /// Stores the file at the given path as a blob, returning its ID.
    #[cfg(feature = "tokio")]
    pub async fn put_file(
        &mut self,
        input_path: impl AsRef<std::path::Path>,
    ) -> Result<Id, RepositoryError> {
        self.put_file_with_options(input_path, PutOptions::default())
            .await
    }

    /// Clones the given source file into a temporary file via a reflink
    /// (copy-on-write clone), without copying any data.
    ///
    /// Returns `None` if the filesystem doesn't support reflinks or the
    /// source resides on a different volume; the caller should fall back to
    /// a regular copy.
    #[cfg(all(feature = "tokio", target_os = "linux"))]
    fn reflink_temp(&self, source: &std::fs::File) -> Option<(String, File)> {
        let (temp_name, temp_file) = self.create_temp_file().ok()?;
        match rustix::fs::ioctl_ficlone(&temp_file, source) {
            Ok(()) => Some((temp_name, temp_file)),
            Err(_) => {
                drop(temp_file);
                let _ = self.0.remove_file(&temp_name);
                None
            },
        }
    }

    /// Clones the given source file into a temporary file via `clonefile`
    /// (copy-on-write clone), without copying any data.
    ///
    /// Returns `None` if the filesystem doesn't support cloning or the
    /// source resides on a different volume; the caller should fall back to
    /// a regular copy.
    #[cfg(all(feature = "tokio", target_vendor = "apple"))]
    fn reflink_temp(&self, source: &std::fs::File) -> Option<(String, File)> {
        use rustix::fs::CloneFlags;

        loop {
            let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let name = std::format!("{}put-{}-{}", TEMP_PREFIX, std::process::id(), sequence);
            match rustix::fs::fclonefileat(source, &self.0, name.as_str(), CloneFlags::empty()) {
                Ok(()) => return self.0.open(&name).ok().map(|file| (name, file)),
                Err(rustix::io::Errno::EXIST) => continue,
                Err(_) => return None,
            }
        }
    }

    /// Reflinks are not supported on this platform.
    #[cfg(all(feature = "tokio", not(any(target_os = "linux", target_vendor = "apple"))))]
    fn reflink_temp(&self, _source: &std::fs::File) -> Option<(String, File)> {
        None
    }

    /// Attempts to store the file at the given path as an uncompressed blob
    /// by reflinking it into the repository, avoiding a data copy.
    ///
    /// Returns `Ok(None)` if reflinking isn't possible (unsupported
    /// filesystem, different volume, etc.), in which case the caller should
    /// fall back to a regular copy. The blob ID is computed from the cloned
    /// file, so a concurrently modified source can't corrupt the store.
    #[cfg(feature = "tokio")]
    fn try_put_file_reflinked(
        &mut self,
        path: &std::path::Path,
        metadata: &ExtendedMetadata,
    ) -> Result<Option<Id>, RepositoryError> {
        let Ok(source) = std::fs::File::open(path) else {
            return Ok(None); // let the fallback path surface the error
        };
        let Some((temp_name, temp_file)) = self.reflink_temp(&source) else {
            return Ok(None);
        };
        drop(source);

        let result: Result<Id, RepositoryError> = (|| {
            // The clone inherits the source's permissions; ensure that the
            // temporary file is writable for the metadata to be attached.
            let mut permissions = temp_file.metadata()?.permissions();
            #[cfg(unix)]
            permissions.set_mode(0o644);
            #[cfg(not(unix))]
            permissions.set_readonly(false);
            temp_file.set_permissions(permissions)?;

            let id = bitcache_core::sync::identify_input(&temp_file)?;
            self.prepare_temp(&temp_name, &temp_file, metadata)?;
            Ok(id)
        })();

        drop(temp_file);
        match result {
            Ok(id) => {
                self.publish_temp(&temp_name, &id, metadata, BlobEncoding::Uncompressed)?;
                Ok(Some(id))
            },
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                Err(error)
            },
        }
    }

    /// Stores the file at the given path as a blob with metadata options.
    ///
    /// When storing uncompressed on a filesystem that supports reflinks
    /// (copy-on-write clones), the file is cloned into the repository
    /// without copying its data. Otherwise, contents are streamed in one
    /// pass. Either way, the blob is published with a hard link, so an
    /// existing blob is never overwritten or replaced.
    #[cfg(feature = "tokio")]
    pub async fn put_file_with_options(
        &mut self,
        input_path: impl AsRef<std::path::Path>,
        options: PutOptions,
    ) -> Result<Id, RepositoryError> {
        use bitcache_core::{
            Hasher,
            tokio::{
                fs::File,
                io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
            },
        };

        let compression = options.compression.unwrap_or(DEFAULT_PUT_COMPRESSION);
        let level = Self::xz_level(compression);
        let encoding = match level {
            Some(_) => BlobEncoding::Xz,
            None => BlobEncoding::Uncompressed,
        };
        let metadata = Self::store_metadata(
            options.expires_nanos(),
            options.media_type().map(ToString::to_string),
        );
        if compression == Compression::None
            && let Some(id) = self.try_put_file_reflinked(input_path.as_ref(), &metadata)?
        {
            return Ok(id);
        }
        let mut input_file = File::open(input_path.as_ref()).await?;
        let (temp_name, temp_file) = self.create_temp_file()?;
        let temp_file = File::from_std(temp_file.into_std());
        let mut writer: std::pin::Pin<std::boxed::Box<dyn AsyncWrite + Send>> = match level {
            Some(level) => std::boxed::Box::pin(
                async_compression::tokio::write::XzEncoder::with_quality(temp_file, level),
            ),
            None => std::boxed::Box::pin(temp_file),
        };

        let result: Result<Id, RepositoryError> = async {
            let mut hasher = Hasher::new();
            let mut buffer = std::vec![0u8; BUFFER_LEN];
            loop {
                match input_file.read(&mut buffer).await? {
                    0 => break,
                    n => {
                        hasher.update(&buffer[..n]);
                        writer.write_all(&buffer[..n]).await?;
                    },
                }
            }
            writer.shutdown().await?;
            Ok(Id(hasher.finalize()))
        }
        .await;

        drop(writer);
        match result {
            Ok(id) => {
                let temp_file = match self.0.open(&temp_name) {
                    Ok(file) => file,
                    Err(error) => {
                        let _ = self.0.remove_file(&temp_name);
                        return Err(error.into());
                    },
                };
                if let Err(error) = self.prepare_temp(&temp_name, &temp_file, &metadata) {
                    drop(temp_file);
                    let _ = self.0.remove_file(&temp_name);
                    return Err(error);
                }
                drop(temp_file);
                self.publish_temp(&temp_name, &id, &metadata, encoding)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(&temp_name);
                Err(error)
            },
        }
    }

    async fn store_bytes(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> Result<Id, RepositoryError> {
        let id = Id::of(&data);
        let metadata = Self::store_metadata(
            options.expires_nanos(),
            options.media_type().map(ToString::to_string),
        );
        let (temp_name, temp_file) = self.create_temp_file()?;

        #[cfg(feature = "tokio")]
        let (result, encoding) = {
            use bitcache_core::tokio::{
                fs::File,
                io::{AsyncWrite, AsyncWriteExt},
            };

            let level = Self::xz_level(options.compression.unwrap_or(DEFAULT_PUT_COMPRESSION));
            let encoding = match level {
                Some(_) => BlobEncoding::Xz,
                None => BlobEncoding::Uncompressed,
            };
            let temp_file = File::from_std(temp_file.into_std());
            let mut writer: std::pin::Pin<std::boxed::Box<dyn AsyncWrite + Send>> = match level {
                Some(level) => std::boxed::Box::pin(
                    async_compression::tokio::write::XzEncoder::with_quality(temp_file, level),
                ),
                None => std::boxed::Box::pin(temp_file),
            };
            let result = async {
                writer.write_all(&data).await?;
                writer.shutdown().await?;
                Ok::<(), RepositoryError>(())
            }
            .await;
            drop(writer);
            (result, encoding)
        };

        #[cfg(not(feature = "tokio"))]
        let (result, encoding) = {
            use std::io::Write;

            let mut temp_file = temp_file;
            (
                temp_file.write_all(&data).map_err(RepositoryError::from),
                BlobEncoding::Uncompressed,
            )
        };

        if result.is_ok() {
            let temp_file = match self.0.open(&temp_name) {
                Ok(file) => file,
                Err(error) => {
                    let _ = self.0.remove_file(&temp_name);
                    return Err(error.into());
                },
            };
            if let Err(error) = self.prepare_temp(&temp_name, &temp_file, &metadata) {
                drop(temp_file);
                let _ = self.0.remove_file(&temp_name);
                return Err(error);
            }
        }

        match result {
            Ok(()) => {
                self.publish_temp(&temp_name, &id, &metadata, encoding)?;
                Ok(id)
            },
            Err(error) => {
                let _ = self.0.remove_file(temp_name);
                Err(error)
            },
        }
    }

    pub async fn put(&mut self, data: Bytes) -> Result<Id, RepositoryError> {
        self.store_bytes(data, PutOptions::default()).await
    }

    /// Best-effort removal of orphaned temporary artifacts (`.tmp-*`) left
    /// behind by interrupted operations.
    ///
    /// Detached `.tmp-clear-*` blob trees are always removed: once renamed
    /// aside, deletion is their only remaining purpose. Temporary files
    /// (e.g., in-flight `.tmp-put-*` stores) are only removed once they are
    /// at least [`TEMP_ORPHAN_TTL`] old, so that stores by concurrent
    /// processes are left undisturbed.
    fn remove_stale_temp_artifacts(&self) {
        let Ok(entries) = self.0.entries() else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name() else {
                continue;
            };
            if !name.starts_with(TEMP_PREFIX) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                let _ = self.0.remove_dir_all(&name);
            } else {
                let orphaned = entry
                    .metadata()
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.into_std().elapsed().ok())
                    .is_some_and(|age| age >= TEMP_ORPHAN_TTL);
                if orphaned {
                    let _ = self.0.remove_file(&name);
                }
            }
        }
    }
}

impl Repository for FsRepository {
    type Error = RepositoryError;

    fn capabilities(&self) -> RepositoryCapabilities {
        self.1
    }

    async fn contains(&self, id: &Id) -> Result<bool, Self::Error> {
        Ok(self.open_live(id)?.is_some())
    }

    async fn get(&self, id: &Id) -> Result<Option<Blob>, Self::Error> {
        let Some(blob) = self.open_live(id)? else {
            return Ok(None);
        };

        #[cfg(feature = "tokio")]
        {
            use bitcache_core::tokio::io::AsyncReadExt;

            let metadata_file = blob.file.try_clone()?;
            let mut reader = BlobFile::new(blob.file, blob.encoding);
            let mut data = Vec::new();
            reader.read_to_end(&mut data).await?;
            let metadata =
                Self::blob_metadata(&metadata_file.metadata()?, blob.extended, data.len() as u64);
            Ok(Some(
                Blob::new_unchecked(id.clone(), data).with_metadata(metadata),
            ))
        }

        #[cfg(not(feature = "tokio"))]
        {
            use std::io::Read;

            if blob.encoding == BlobEncoding::Xz {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "XZ blob reads require the tokio feature",
                )
                .into());
            }
            let mut file = blob.file;
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            let metadata = Self::blob_metadata(&file.metadata()?, blob.extended, data.len() as u64);
            Ok(Some(
                Blob::new_unchecked(id.clone(), data).with_metadata(metadata),
            ))
        }
    }

    async fn get_len(&self, id: &Id) -> Result<Option<u64>, Self::Error> {
        let Some(blob) = self.open_live(id)? else {
            return Ok(None);
        };
        if blob.encoding == BlobEncoding::Uncompressed {
            return Ok(Some(blob.file.metadata()?.len()));
        }

        #[cfg(feature = "tokio")]
        {
            use bitcache_core::tokio::io::AsyncReadExt;

            let mut reader = BlobFile::new(blob.file, blob.encoding);
            let mut buffer = [0u8; BUFFER_LEN];
            let mut len = 0u64;
            loop {
                let read = reader.read(&mut buffer).await?;
                if read == 0 {
                    break;
                }
                len = len.checked_add(read as u64).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "blob length overflow")
                })?;
            }
            Ok(Some(len))
        }

        #[cfg(not(feature = "tokio"))]
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "XZ blob reads require the tokio feature",
        )
        .into())
    }

    async fn put(&mut self, data: Bytes) -> Result<Id, Self::Error> {
        self.put(data).await
    }

    async fn put_with_options(
        &mut self,
        data: Bytes,
        options: PutOptions,
    ) -> Result<Id, Self::Error> {
        self.store_bytes(data, options).await
    }

    #[cfg(feature = "tokio")]
    async fn put_from_path(
        &mut self,
        path: &std::path::Path,
        options: PutOptions,
    ) -> Result<Id, Self::Error> {
        self.put_file_with_options(path, options).await
    }

    async fn remove(&mut self, id: &Id) -> Result<bool, Self::Error> {
        let Some(blob) = self.open_live(id)? else {
            return Ok(false);
        };
        drop(blob.file);

        let mut removed = false;
        for encoding in [BlobEncoding::Uncompressed, BlobEncoding::Xz] {
            match self.0.remove_file(Self::path_for(id, encoding)) {
                Ok(()) => removed = true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(removed)
    }

    async fn set_expiry(
        &mut self,
        id: &Id,
        expires_nanos: Option<u64>,
    ) -> Result<bool, Self::Error> {
        let Some(mut blob) = self.open_live(id)? else {
            return Ok(false);
        };
        blob.extended.expires = expires_nanos;
        blob.extended.updated = None;
        self.write_existing_extended(&blob.name, &blob.file, &blob.extended)
    }

    async fn set_media_type(
        &mut self,
        id: &Id,
        media_type: Option<&str>,
    ) -> Result<bool, Self::Error> {
        let Some(mut blob) = self.open_live(id)? else {
            return Ok(false);
        };
        blob.extended.media_type = media_type.map(ToString::to_string);
        blob.extended.updated = None;
        self.write_existing_extended(&blob.name, &blob.file, &blob.extended)
    }

    async fn compact(&mut self) -> Result<(), Self::Error> {
        self.compact_with_options(CompactOptions::default()).await
    }

    async fn compact_with_options(&mut self, options: CompactOptions) -> Result<(), Self::Error> {
        // Compaction doubles as maintenance: sweep any orphaned temporary
        // artifacts left behind by interrupted operations.
        self.remove_stale_temp_artifacts();

        #[cfg(feature = "tokio")]
        {
            let mut cursor = self.cursor(false);
            let mut previous: Option<Id> = None;
            while let Some(item) = cursor.next().await {
                let (id, _encoding) = item?;
                if previous.as_ref() == Some(&id) {
                    continue;
                }
                self.compact_blob(&id, options.compression).await?;
                previous = Some(id);
            }
            Ok(())
        }

        #[cfg(not(feature = "tokio"))]
        {
            let _ = options;
            Ok(())
        }
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        // Atomically detach the whole blobs tree by renaming it aside, then
        // delete it offline. Readers holding an open cursor keep iterating
        // their pinned (renamed) generation; new operations see an empty
        // repository immediately.
        let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let trash = std::format!("{}clear-{}-{}", TEMP_PREFIX, std::process::id(), sequence);
        match self.0.rename(BLOBS_DIR, &self.0, &trash) {
            Ok(()) => {
                // Best effort: recreate an empty blobs directory right away.
                // This races with concurrent writers, which lazily recreate
                // it themselves, so a failure here is harmless.
                let _ = self.0.create_dir(BLOBS_DIR);
                self.0.remove_dir_all(&trash)?;
            },
            // Nothing to clear; still ensure the blobs directory exists.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let _ = self.0.create_dir(BLOBS_DIR);
            },
            Err(error) => return Err(error.into()),
        }
        self.remove_stale_temp_artifacts();
        Ok(())
    }

    fn list(&self, options: ListOptions) -> impl Stream<Item = Result<Id, Self::Error>> {
        // The cursor visits one shard subdirectory at a time in ID order, so
        // the stream is ordered while memory use stays bounded by shard size.
        let descending = options.order == Some(ListOrder::Descending);
        let limit = options.limit.unwrap_or(usize::MAX);
        self.cursor(descending)
            .scan(
                (None::<Id>, false),
                move |(previous, failed), item| {
                    if *failed {
                        return core::future::ready(None);
                    }
                    let item = match item {
                        Err(error) => {
                            *failed = true;
                            Some(Err(error))
                        },
                        // Skip the duplicate entry when a blob exists in both
                        // encodings (entries for one ID are adjacent).
                        Ok((id, _encoding)) if previous.as_ref() == Some(&id) => None,
                        Ok((id, _encoding)) => {
                            *previous = Some(id.clone());
                            if !options.matches(&id) {
                                None
                            } else {
                                match self.open_live(&id) {
                                    Ok(Some(_)) => Some(Ok(id)),
                                    Ok(None) => None,
                                    Err(error) => {
                                        *failed = true;
                                        Some(Err(error))
                                    },
                                }
                            }
                        },
                    };
                    core::future::ready(Some(item))
                },
            )
            .filter_map(core::future::ready)
            .take(limit)
    }
}
