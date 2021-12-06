//! Aztec's MPC ignition ceremony, there are 100.8 million BN254 points
//! generated. For concrete details: https://github.com/AztecProtocol/ignition-verification
use anyhow::{bail, Result};
use ark_bn254::{Bn254, Fq, Fq2, G1Affine, G2Affine};
use ark_ec::AffineCurve;
use ark_ff::{BigInteger256, PrimeField};
use ark_poly_commit::kzg10::{self, UniversalParams};
use ark_std::{
    collections::BTreeMap,
    format,
    fs::File,
    io::{Read, Seek, SeekFrom},
    vec,
    vec::Vec,
};

const NUM_TRANSCRIPTS: usize = 1;
const TRANSCRIPT_DIR: &'static str = "./data/aztec20";
const NUM_G1_PER_TRANSCRIPT: usize = 5_040_000;
const NUM_G2: usize = 2;
const G1_STARTING_POS: u64 = 28; // pos of the first G1 points in transcript file
const NUM_BIGINT_PER_G1: usize = 2;
const NUM_BIGINT_PER_G2: usize = 4;

/// Retreive public parameters when given as input the maximum degree degree for
/// the polynomial commitment scheme.
/// This API is similar to [KZG10::setup][setup]
///
/// [setup]: https://docs.rs/ark-poly-commit/0.3.0/ark_poly_commit/kzg10/struct.KZG10.html#method.setup
pub fn get_crs(
    max_degree: usize,
    produce_g2_powers: bool,
) -> Result<kzg10::UniversalParams<Bn254>> {
    if max_degree < 1 {
        bail!("Max degree has to be >= 1");
    }

    let mut powers_of_g = vec![G1Affine::prime_subgroup_generator()];
    powers_of_g.extend_from_slice(&parse_g1_points(max_degree)?);

    // TODO: used for Marlin's PCS variant, see Marlin19 Appendix B
    // currently not used for vanilla KZG.
    let powers_of_gamma_g = BTreeMap::new();

    let [h, beta_h] = parse_g2_points()?;

    let neg_powers_of_h = if produce_g2_powers {
        // TODO: add impl for this
        BTreeMap::new()
    } else {
        BTreeMap::new()
    };

    let prepared_h = h.into();
    let prepared_beta_h = beta_h.into();

    let pp = UniversalParams {
        powers_of_g,
        powers_of_gamma_g,
        h,
        beta_h,
        neg_powers_of_h,
        prepared_h,
        prepared_beta_h,
    };
    Ok(pp)
    // unimplemented!();
}

// Returns x.[1], x^2.[1], ... , x^`bound`.[1] where `x` is toxic
// waste/trapdoor, [1] is G1 generator (1, 2).
fn parse_g1_points(bound: usize) -> Result<Vec<G1Affine>> {
    if bound > NUM_TRANSCRIPTS * NUM_G1_PER_TRANSCRIPT {
        bail!("Aztec ceremoy only supports up to 100.8 million degree.");
    }
    let mut g1_points = Vec::new();
    let num_full_transcript = bound / NUM_G1_PER_TRANSCRIPT;
    let remainder_num_points = bound - num_full_transcript * NUM_G1_PER_TRANSCRIPT;

    for file_idx in 0..num_full_transcript {
        let mut f = File::open(format!("{}/transcript{:02}.dat", TRANSCRIPT_DIR, file_idx))?;
        g1_points.extend_from_slice(&parse_g1_points_from_file(&mut f, NUM_G1_PER_TRANSCRIPT)?);
    }
    let mut f = File::open(format!(
        "{}/transcript{:02}.dat",
        TRANSCRIPT_DIR,
        num_full_transcript + 1
    ))?;
    g1_points.extend_from_slice(&parse_g1_points_from_file(&mut f, remainder_num_points)?);

    Ok(g1_points)
}

// Parse G1Affine points from CRS
// Concrete format spec:
// https://github.com/AztecProtocol/ignition-verification/blob/master/Transcript_spec.md#structure-of-a-transcript-file
fn parse_g1_points_from_file(f: &mut File, num_points: usize) -> Result<Vec<G1Affine>> {
    let mut g1_points = Vec::new();

    if num_points > NUM_G1_PER_TRANSCRIPT {
        bail!("Internal Error, should not retrieve more than 5 million points per file");
    }

    for i in 0..num_points {
        // [X, Y]
        let mut bigints_repr = [[0u64; 4]; NUM_BIGINT_PER_G1];
        for j in 0..NUM_BIGINT_PER_G1 {
            for k in 0..4 {
                f.seek(SeekFrom::Start(
                    G1_STARTING_POS + i as u64 * 64 + j as u64 * 32 + k * 8,
                ))?;
                let mut buf = [0u8; 8];
                f.read_exact(&mut buf)?;
                bigints_repr[j as usize][k as usize] = u64::from_be_bytes(buf);
            }
        }

        let x = Fq::from_repr(BigInteger256::new(bigints_repr[0]))
            .expect("Failed to parse G1 point's x-coordinate");
        let y = Fq::from_repr(BigInteger256::new(bigints_repr[1]))
            .expect("Failed to parse G1 point's y-coordinate");

        g1_points.push(G1Affine::new(x, y, false));
    }
    Ok(g1_points)
}

// Parse G2Affine points from CRS
// Concrete format spec:
// https://github.com/AztecProtocol/ignition-verification/blob/master/Transcript_spec.md#structure-of-a-transcript-file
fn parse_g2_points() -> Result<[G2Affine; NUM_G2]> {
    let mut g2_points = [G2Affine::default(); NUM_G2];

    // Parse 2 G2Affine points from CRS
    let mut f = File::open(format!("{}/transcript00.dat", TRANSCRIPT_DIR))?;
    for i in 0..NUM_G2 {
        // [x.c0, x.c1, y.c0, y.c1]
        let mut bigints_repr = [[0u64; 4]; NUM_BIGINT_PER_G2];

        for j in 0..NUM_BIGINT_PER_G2 {
            for k in 0..4 {
                // size of G1 is
                // size of G2 is 128 bytes (4 * 32).
                // size of a bigint is 32 bytes.
                // size of u64 is 8 bytes.
                f.seek(SeekFrom::Start(
                    G1_STARTING_POS
                        + NUM_G1_PER_TRANSCRIPT as u64 * 64
                        + i as u64 * 128
                        + j as u64 * 32
                        + k * 8,
                ))?;
                let mut buf = [0u8; 8];
                f.read_exact(&mut buf)?;
                bigints_repr[j as usize][k as usize] = u64::from_be_bytes(buf);
            }
        }
        let x_c0 = Fq::from_repr(BigInteger256::new(bigints_repr[0]))
            .expect("Failed to parse G2 point's X.c0");
        let x_c1 = Fq::from_repr(BigInteger256::new(bigints_repr[1]))
            .expect("Failed to parse G2 point's X.c1");
        let y_c0 = Fq::from_repr(BigInteger256::new(bigints_repr[2]))
            .expect("Failed to parse G2 point's Y.c0");
        let y_c1 = Fq::from_repr(BigInteger256::new(bigints_repr[3]))
            .expect("Failed to parse G2 point's Y.c1");
        let x = Fq2::new(x_c0, x_c1);
        let y = Fq2::new(y_c0, y_c1);

        g2_points[i] = G2Affine::new(x, y, false);
    }

    Ok(g2_points)
}

#[test]
fn test_aztec_crs() {
    // TODO: add test
}
