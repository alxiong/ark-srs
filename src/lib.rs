//! Parsing trusted setup CRS into arkwork compatible rust code.

#![deny(missing_docs)]
#![no_std]

#[cfg(feature = "kzg-aztec")]
pub mod aztec20;
pub mod load;

extern crate alloc;
extern crate std;
