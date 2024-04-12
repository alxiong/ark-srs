//! Configurations and constants, centralized for single responsibility

use hex_literal::hex;

/// data related to Aztec's ignition ceremony (including original transcripts
/// from the ceremony and the arkworks serialized data blobs)
pub(crate) const AZTEC20_DIR: &str = "./data/aztec20";

/// List of pre-computed arkworks-serialized parameter files, storing their
/// `(degree, sha256sum)`
pub const AZTEC20_CHECKSUMS: [(usize, [u8; 32]); 8] = [
    (
        1024,
        hex!("0e2a5fb1d9102ee5b06723472b23f4f29f938712251a7b5b75eed4df4049871c"),
    ),
    (
        16392,
        hex!("386d24773c0ccffd7c7510ae92897c53cf9f8491cc75a82f62d3384e8a259bcb"),
    ),
    (
        32776,
        hex!("c755e391fb004ce3dec8b2f2bd40c8b079258a01fcea279541c571947742c990"),
    ),
    (
        65544,
        hex!("8d4a6edca593f3d2a0ff89b4a2e604dcf4820b0365c051729231f0795d30828a"),
    ),
    (
        131080,
        hex!("41ebc2209b873f24c5b5641ba38b1160e7c6dda7479f2a0c39b8ab8fa6fbd219"),
    ),
    (
        262152,
        hex!("f3b8f18997861e993d5a3ff8e524df197b1ae4fe10d76331166ede48e49b0a13"),
    ),
    (
        524296,
        hex!("44dc8de251c635d12447de018f5148952b0be49230119a10eb814b5355e0b9da"),
    ),
    (
        1048584,
        hex!("cded83e82e4b49fee4cb2e0f374f996954fe12548ad39100432ee493069ef09d"),
    ),
];
