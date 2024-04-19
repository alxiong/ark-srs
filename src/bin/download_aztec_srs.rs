//! Download all Aztec SRS and save them locally

use ark_srs::kzg10::aztec20::setup;

fn main() {
    tracing_subscriber::fmt::init();

    let degrees = [
        1024, 16_392, 32_776, 65_544, 131_080, 262_152, 524_296, 1_048_584,
    ];

    for degree in degrees {
        setup(degree).expect("download SRS succeeds");
    }
}
