//! Load Aztec SRS and save to files

fn main() {
    ark_srs::load::kzg10::bn254::aztec::store_aztec_srs(None).unwrap();
}
