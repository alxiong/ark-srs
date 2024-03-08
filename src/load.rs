//! Utils for persisting serialized data to files and loading them into memroy.
//! We deal with `ark-serialize::CanonicalSerialize` compatible objects.

#![allow(unused_imports)]
use std::{env, fs::File, io::BufReader, path::PathBuf};

use alloc::{format, vec::Vec};
use anyhow::{anyhow, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Read, Write};

/// store any serializable data into `dest`.
pub(crate) fn store_data<T: CanonicalSerialize>(data: T, dest: PathBuf) -> Result<()> {
    let mut f = File::create(dest)?;
    let mut bytes = Vec::new();
    data.serialize_uncompressed(&mut bytes)?;
    Ok(f.write_all(&bytes)?)
}

/// load any deserializable data into memory
#[allow(dead_code)]
pub(crate) fn load_data<T: CanonicalDeserialize>(src: PathBuf) -> Result<T> {
    let f = File::open(src)?;
    // maximum 8 KB of buffer for memory exhaustion protection for malicious file
    let mut reader = BufReader::with_capacity(8000, f);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;

    Ok(T::deserialize_uncompressed_unchecked(&bytes[..])?)
}

/// return the directory containing the Cargo.toml (i.e. current project root)
pub(crate) fn get_project_root() -> Result<PathBuf> {
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

        #[cfg(feature = "kzg-aztec")]
        /// Aztec2020 KZG setup
        pub mod aztec {
            use super::*;
            /// max supported degree to load from pre-serialized parameter
            /// files.
            pub const MAX_SUPPORTED_DEGREE: usize = 1_048_600;

            /// Load SRS from Aztec's ignition ceremony from files.
            pub fn load_aztec_srs(degree: usize) -> Result<kzg10::UniversalParams<Bn254>> {
                let mut srs;
                if degree > MAX_SUPPORTED_DEGREE {
                    return Err(anyhow!("Too large for cached SRS files"));
                } else {
                    let bytes = include_bytes!("../data/aztec20/kzg10-bn254-aztec-srs-1048600.bin");
                    srs = kzg10::UniversalParams::<Bn254>::deserialize_uncompressed_unchecked(
                        &bytes[..],
                    )?;
                }

                // trim the srs to fit the actual requested degree
                srs.powers_of_g.truncate(degree + 1);
                Ok(srs)
            }

            /// Store SRS into files into `dest` directory
            pub fn store_aztec_srs(dest: Option<PathBuf>) -> Result<()> {
                let mut srs = crate::aztec20::kzg10_setup(MAX_SUPPORTED_DEGREE)?;

                srs.powers_of_g.truncate(MAX_SUPPORTED_DEGREE + 1);

                let dest = match dest {
                    Some(ref d) => d.clone(),
                    None => {
                        let mut path = get_project_root()?;
                        path.push("data");
                        path.push("aztec20");
                        path.push(format!("kzg10-bn254-aztec-srs-{}", MAX_SUPPORTED_DEGREE));
                        path.set_extension("bin");
                        path
                    },
                };

                store_data(srs, dest)?;
                Ok(())
            }
        }
    }
}
