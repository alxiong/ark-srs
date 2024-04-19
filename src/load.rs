//! Utils for persisting serialized data to files and loading them into memroy.
//! We deal with `ark-serialize::CanonicalSerialize` compatible objects.

use alloc::{
    borrow::ToOwned,
    format,
    string::{String, ToString},
    vec::Vec,
};
use anyhow::{anyhow, Context, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Read, Write};
use ark_std::rand::{distributions::Alphanumeric, Rng as _};
use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, create_dir_all, File},
    io::BufReader,
    path::{Path, PathBuf},
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

/// Download srs file and save to disk
///
/// - `basename`: the filename used in download URL
/// - `dest`: the filename for local cache
pub fn download_srs_file(basename: &str, dest: impl AsRef<Path>) -> Result<()> {
    // Ensure download directory exists
    create_dir_all(dest.as_ref().parent().context("no parent dir")?)
        .context("Unable to create directory")?;

    let version = "0.2.0"; // TODO infer or make configurable
    let url = format!(
        "https://github.com/EspressoSystems/ark-srs/releases/download/v{version}/{basename}",
    );
    tracing::info!("Downloading SRS from {url}");
    let mut buf: Vec<u8> = Vec::new();
    ureq::get(&url).call()?.into_reader().read_to_end(&mut buf)?;

    // Download to a temporary file and rename to dest on completion. This
    // should prevent some errors if this function is called concurrently
    // because the concurrent operations would happen on different files and the
    // destination file should never be in an incomplete state.
    let mut temp_path = dest.as_ref().as_os_str().to_owned();
    let suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    temp_path.push(format!(".temp.{suffix}"));
    {
        let mut f = File::create(&temp_path)?;
        f.write_all(&buf)?;
    }
    std::fs::rename(temp_path, dest.as_ref())?;
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
                format!("kzg10-aztec20-srs-{degree}.bin").to_string()
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
