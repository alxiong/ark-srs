//! Utils for persisting serialized data to files and loading them into memory.
//! We deal with `ark-serialize::CanonicalSerialize` compatible objects.

use alloc::{borrow::ToOwned, format, string::String, vec::Vec};
use anyhow::{anyhow, Context, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Read, Write};
use directories::ProjectDirs;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, create_dir_all, File, OpenOptions},
    io::BufReader,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

/// store any serializable data into `dest`.
pub fn store_data<T: CanonicalSerialize>(data: T, dest: PathBuf) -> Result<()> {
    let mut f = File::create(dest)?;
    let mut bytes = Vec::new();
    data.serialize_uncompressed(&mut bytes)?;
    Ok(f.write_all(&bytes)?)
}

/// load any deserializable data into memory
pub fn load_data<T: CanonicalDeserialize>(src: PathBuf) -> Result<T> {
    let f = File::open(src)?;
    // maximum 8 KB of buffer for memory exhaustion protection for malicious file
    let mut reader = BufReader::with_capacity(8000, f);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;

    Ok(T::deserialize_uncompressed_unchecked(&bytes[..])?)
}

/// Download `url` to `dest` with retry, exponential backoff, and
/// concurrent-download deduplication via file locking.
///
/// A `.part` file is used as both a temporary staging file and an `flock`
/// target so that multiple threads/processes downloading the same `dest`
/// are serialised: only the first caller performs the actual download while
/// the rest block on the lock and return once the file appears.
pub(crate) fn download_url_to_file(
    url: &str,
    dest: &Path,
    max_retries: usize,
    base_backoff: Duration,
) -> Result<()> {
    // Fast path: if the destination already exists, skip everything.
    if dest.exists() {
        return Ok(());
    }

    create_dir_all(dest.parent().context("no parent dir")?)
        .context("Unable to create directory")?;

    // .part file serves double duty: temp file for atomic rename and flock
    // target to deduplicate concurrent downloads. Not truncated on open so a
    // second opener doesn't clobber an in-progress write. Left on disk after
    // failure -- harmless, the next caller overwrites it.
    let part_path = {
        let mut p = dest.as_os_str().to_owned();
        p.push(".part");
        PathBuf::from(p)
    };
    let part_file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&part_path)?;
    part_file.lock_exclusive()?;

    // Another thread may have completed the download while we blocked on the lock.
    if dest.exists() {
        return Ok(());
    }

    // Use an agent with explicit timeouts so a hung connection cannot hold the
    // exclusive lock indefinitely.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(300))
        .build();

    let mut last_err = None;
    for attempt in 0..=max_retries {
        match agent.get(url).call() {
            Ok(resp) => {
                // Stream the response directly into the .part file to keep
                // memory usage bounded for large SRS assets.
                part_file.set_len(0)?;
                use std::io::Seek;
                (&part_file).seek(std::io::SeekFrom::Start(0))?;
                let bytes_copied = std::io::copy(&mut resp.into_reader(), &mut &part_file)?;
                if bytes_copied == 0 {
                    last_err = Some(anyhow!("zero-byte response"));
                } else {
                    fs::rename(&part_path, dest)?;
                    return Ok(());
                }
            },
            Err(e) => last_err = Some(anyhow::Error::from(e)),
        }

        if attempt < max_retries {
            let backoff = base_backoff * 2u32.saturating_pow(attempt as u32);
            thread::sleep(backoff);
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow!("download failed")))
}

/// Download srs file and save to disk
///
/// - `basename`: the filename used in download URL
/// - `dest`: the filename for local cache
pub fn download_srs_file(basename: &str, dest: impl AsRef<Path>) -> Result<()> {
    let version = "0.2.0"; // TODO infer or make configurable
    let url = format!(
        "https://github.com/EspressoSystems/ark-srs/releases/download/v{version}/{basename}",
    );
    tracing::info!("Downloading SRS from {url}");
    download_url_to_file(&url, dest.as_ref(), 5, Duration::from_secs(1))?;
    tracing::info!("Saved SRS to {:?}", dest.as_ref());
    Ok(())
}

/// The base data directory for the project
fn get_project_root() -> Result<PathBuf> {
    // (empty) qualifier, (empty) organization, and application name
    // see more <https://docs.rs/directories/5.0.1/directories/struct.ProjectDirs.html#method.from>
    Ok(ProjectDirs::from("", "", "ark-srs")
        .context("Failed to get project root")?
        .data_dir()
        .to_path_buf())
}

/// loading KZG10 parameters from files
pub mod kzg10 {
    use super::*;
    use ark_poly_commit::kzg10;

    /// ceremonies for curve [Bn254][https://docs.rs/ark-bn254/latest/ark_bn254/]
    pub mod bn254 {
        use super::*;
        use ark_bn254::Bn254;

        /// Aztec2020 KZG setup
        pub mod aztec {
            use crate::constants::AZTEC20_CHECKSUMS;

            use super::*;

            /// Returns the default path for pre-serialized param files
            pub fn default_path(project_root: Option<PathBuf>, degree: usize) -> Result<PathBuf> {
                let mut path = if let Some(root) = project_root {
                    root
                } else {
                    get_project_root()?
                };
                path.push("aztec20");
                path.push(degree_to_basename(degree));
                path.set_extension("bin");
                Ok(path)
            }

            pub(crate) fn degree_to_basename(degree: usize) -> String {
                format!("kzg10-aztec20-srs-{degree}.bin")
            }

            /// Load SRS from Aztec's ignition ceremony from files.
            ///
            /// # Note
            /// we force specifying a `src` (instead of taking in `Option`) in
            /// case the param files contains much more than `degree` needed.
            /// And we want to avoid unnecessarily complicated logic for
            /// iterating through all parameter files and find the smallest
            /// param files that's bigger than the degree requested.
            pub fn load_aztec_srs(
                degree: usize,
                src: PathBuf,
            ) -> Result<kzg10::UniversalParams<Bn254>> {
                let mut f = File::open(&src).map_err(|_| anyhow!("{} not found", src.display()))?;
                // the max degree of the param file supported, parsed from file name
                // getting the 1024 out of `data/aztec20/kzg10-aztec20-srs-1024.bin`
                let f_degree = src
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .rsplit_once('-')
                    .expect("unconventional filename")
                    .1
                    .parse::<usize>()
                    .expect("fail to parse to uint");

                let mut bytes = Vec::new();
                f.read_to_end(&mut bytes)?;

                let checksum: [u8; 32] = Sha256::digest(&bytes).into();
                if !AZTEC20_CHECKSUMS
                    .iter()
                    .any(|(d, cksum)| *d == f_degree && checksum == *cksum)
                {
                    tracing::error!("Checksum failed, removing {}", src.display());
                    fs::remove_file(src)?;
                    return Err(anyhow!("Checksum failed!"));
                }

                let mut srs = kzg10::UniversalParams::<Bn254>::deserialize_uncompressed_unchecked(
                    &bytes[..],
                )?;

                // trim the srs to fit the actual requested degree
                srs.powers_of_g.truncate(degree + 1);
                Ok(srs)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    #[test]
    fn test_download_success() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_body("hello")
            .expect(1)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        let url = format!("{}/file.bin", server.url());

        download_url_to_file(&url, &dest, 0, Duration::from_millis(10)).unwrap();

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "hello");
        mock.assert();
    }

    #[test]
    fn test_retry_succeeds_after_transient_failures() {
        let mut server = mockito::Server::new();
        let _fallback = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_body("ok")
            .create();
        let _failures = server
            .mock("GET", "/file.bin")
            .with_status(500)
            .expect(2)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        let url = format!("{}/file.bin", server.url());

        download_url_to_file(&url, &dest, 2, Duration::from_millis(10)).unwrap();

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "ok");
    }

    #[test]
    fn test_retry_exhausted_returns_error() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/file.bin")
            .with_status(500)
            .expect(3)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        let url = format!("{}/file.bin", server.url());

        let result = download_url_to_file(&url, &dest, 2, Duration::from_millis(10));

        assert!(result.is_err());
        assert!(!dest.exists());
    }

    #[test]
    fn test_concurrent_downloads_deduplicated() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_body("concurrent")
            .expect(1)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        let url = format!("{}/file.bin", server.url());

        let barrier = Arc::new(Barrier::new(5));
        let handles: Vec<_> = (0..5)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let url = url.clone();
                let dest = dest.clone();
                thread::spawn(move || {
                    barrier.wait();
                    download_url_to_file(&url, &dest, 0, Duration::from_millis(10))
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap().unwrap();
        }

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "concurrent");
        mock.assert();
    }

    #[test]
    fn test_existing_file_skipped_under_lock() {
        let mut server = mockito::Server::new();
        let mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .expect(0)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        std::fs::write(&dest, "existing").unwrap();

        let url = format!("{}/file.bin", server.url());

        download_url_to_file(&url, &dest, 0, Duration::from_millis(10)).unwrap();

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "existing");
        mock.assert();
    }

    #[test]
    fn test_part_file_cleaned_up_on_success() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_body("data")
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");
        let url = format!("{}/file.bin", server.url());

        download_url_to_file(&url, &dest, 0, Duration::from_millis(10)).unwrap();

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "data");
        let mut part_path = dest.as_os_str().to_owned();
        part_path.push(".part");
        assert!(!PathBuf::from(part_path).exists());
    }

    #[test]
    fn test_stale_part_file_does_not_prevent_download() {
        let mut server = mockito::Server::new();
        let _mock = server
            .mock("GET", "/file.bin")
            .with_status(200)
            .with_body("fresh")
            .expect(1)
            .create();

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("file.bin");

        let mut part_path = dest.as_os_str().to_owned();
        part_path.push(".part");
        std::fs::write(PathBuf::from(&part_path), "stale data").unwrap();

        let url = format!("{}/file.bin", server.url());
        download_url_to_file(&url, &dest, 0, Duration::from_millis(10)).unwrap();

        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "fresh");
    }
}
