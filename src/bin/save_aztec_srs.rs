//! Load Aztec SRS and save to files

fn main() {
    let degrees = [1024];

    for degree in degrees {
        print!("Parsing SRS for degree {degree} ...");
        let mut srs = ark_srs::kzg10::aztec20::setup_from_raw(degree).unwrap();
        print!(" done.\n");
        srs.powers_of_g.truncate(degree + 1);

        let dest = ark_srs::load::kzg10::bn254::aztec::default_path(degree).unwrap();
        print!("Saving to {} ...", dest.display());
        ark_srs::load::store_data(srs, dest).unwrap();
        print!(" done.\n");
    }
}
