//! Utils for persisting serialized data to files and loading them into memroy.
//! We deal with `ark-serialize::CanonicalSerialize` compatible objects.

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
        use crate::aztec20;

        use super::*;
        use anyhow::anyhow;
        use ark_bn254::Bn254;

        /// supported degrees to load from pre-serialized parameter files.
        pub const SUPPORTED_DEGREES: [usize; 4] = [32, 1024, 32768, 131072];

        /// Load SRS from Aztec's ignition ceremony from files, we support size:
        /// `[2^5, 2^10, 2^15, 2^17]`, these are loaded faster than Aztec's
        /// transcript.
        ///
        /// This function supports any `degree<=2^17`.
        pub fn load_aztec_srs(
            degree: usize,
            src: Option<PathBuf>,
        ) -> Result<kzg10::UniversalParams<Bn254>> {
            if degree as u64 > 2u64.pow(17) {
                return Err(anyhow!(
                    "Large degree, please use `crs::aztec20::kzg10_setup()` instead!"
                ));
            }

            let target_degree = SUPPORTED_DEGREES
                .iter()
                .filter(|&x| x >= &degree)
                .min()
                .unwrap(); // shouldn't panic

            let src = match src {
                Some(s) => s,
                None => {
                    let mut path = get_project_root()?;
                    path.push("data");
                    path.push(format!("kzg10-bn254-aztec-srs-{}", target_degree));
                    path.set_extension("bin");
                    path
                },
            };
            let mut srs: kzg10::UniversalParams<Bn254> = load_data(src)?;

            // trim the srs to fit the actual requested degree
            srs.powers_of_g.truncate(degree + 1);
            Ok(srs)
        }

        /// Store SRS into files into `dest` directory
        pub fn store_aztec_srs(dest: Option<PathBuf>) -> Result<()> {
            let max_degree = *SUPPORTED_DEGREES.iter().max().unwrap();
            let srs = aztec20::kzg10_setup(max_degree)?;

            for degree in SUPPORTED_DEGREES {
                let mut trim_srs = srs.clone();
                trim_srs.powers_of_g.truncate(degree + 1);

                let dest = match dest {
                    Some(ref d) => d.clone(),
                    None => {
                        let mut path = get_project_root()?;
                        path.push("data");
                        path.push(format!("kzg10-bn254-aztec-srs-{}", degree));
                        path.set_extension("bin");
                        path
                    },
                };

                store_data(trim_srs, dest)?;
            }
            Ok(())
        }
    }
}
