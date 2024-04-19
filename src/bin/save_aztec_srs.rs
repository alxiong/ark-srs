//! Load Aztec SRS and save to files

fn main() {
    // instead of perfect power_of_two, we include a few more points
    // because many variant requires slightly more for masking or optimization
    // reasons. You can choose any degree that's slightly larger than your
    // instance upper bound.
    let degrees = [
        1024, 16_392, 32_776, 65_544, 131_080, 262_152, 524_296, 1_048_584,
    ];

    for degree in degrees {
        print!("Parsing SRS for degree {degree} ...");
        let mut srs = ark_srs::kzg10::aztec20::setup_from_raw(degree).unwrap();
        print!(" done.\n");
        srs.powers_of_g.truncate(degree + 1);

        let dest = ark_srs::load::kzg10::bn254::aztec::default_path(None, degree).unwrap();
        print!("Saving to {} ...", dest.display());
        ark_srs::load::store_data(srs, dest).unwrap();
        print!(" done.\n");
    }
}
