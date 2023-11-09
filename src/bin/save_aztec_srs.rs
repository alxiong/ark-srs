//! Load Aztec SRS and save to files
use crs::load;

fn main() {
    load::kzg10::bn254::store_aztec_srs(None).unwrap();
}
