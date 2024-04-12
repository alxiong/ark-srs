//! Utils for persisting serialized data to files and loading them into memroy.
//! We deal with `ark-serialize::CanonicalSerialize` compatible objects.

use alloc::{format, vec::Vec};
use anyhow::{anyhow, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Read, Write};
use sha2::{Digest, Sha256};
use std::{env, fs::File, io::BufReader, path::PathBuf};

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

/// return the directory containing the Cargo.toml (i.e. current project root)
pub fn get_project_root() -> Result<PathBuf> {
    let mut path = env::current_exe()?;
    // move up one level and start searching for `Cargo.toml` file
    path.pop();
    while !path.join("Cargo.toml").exists() {
        if !path.pop() {
            return Err(anyhow!("Not running in a cargo project."));
        }
    }
    Ok(path)
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
            pub fn default_path(degree: usize) -> Result<PathBuf> {
                let mut path = get_project_root()?;
                path.push("data");
                path.push("aztec20");
                path.push(format!("kzg10-aztec20-srs-{}", degree));
                path.set_extension("bin");
                Ok(path)
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
                    return Err(anyhow!("checksum mismatched!"));
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
