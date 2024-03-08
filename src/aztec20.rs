//! Aztec's MPC ignition ceremony, there are 100.8 million BN254 points
//! generated. For concrete details: https://github.com/AztecProtocol/ignition-verification

use anyhow::{bail, Result};
use ark_bn254::{Bn254, Fq, Fq2, G1Affine, G2Affine};
use ark_ec::AffineRepr;
use ark_ff::{BigInteger256, PrimeField};
use ark_poly_commit::kzg10::UniversalParams;
use ark_std::{
    collections::BTreeMap,
    format,
    fs::File,
    io::{Read, Seek, SeekFrom},
    vec,
    vec::Vec,
};

use crate::load::kzg10::bn254::aztec::load_aztec_srs;

const TRANSCRIPT_DIR: &str = "./data/aztec20";
const NUM_TRANSCRIPTS: usize = 20;
const NUM_G1_PER_TRANSCRIPT: usize = 5_040_000;
const NUM_G2: usize = 2;
const G1_STARTING_POS: u64 = 28; // pos of the first G1 points in transcript file
const NUM_BIGINT_PER_G1: usize = 2;
const NUM_BIGINT_PER_G2: usize = 4;

/// Retreive public parameters when given as input the maximum degree degree for
/// the polynomial commitment scheme.
/// This API is similar to [KZG10::setup][setup]
///
/// [setup]: https://docs.rs/ark-poly-commit/0.4.0/ark_poly_commit/kzg10/struct.KZG10.html#method.setup
pub fn kzg10_setup(supported_degree: usize) -> Result<UniversalParams<Bn254>> {
    if !(1..=NUM_G1_PER_TRANSCRIPT * NUM_TRANSCRIPTS).contains(&supported_degree) {
        bail!("Max degree has to be between [1, 100.8 million].");
    }

    // first try to load from precomputed/serialized binary files, if failed, then
    // proceed to use the original transcript which is much larger and slower to
    // parse.
    let pp = load_aztec_srs(supported_degree);
    if pp.is_ok() {
        return pp;
    }

    let mut powers_of_g = vec![G1Affine::generator()];
    powers_of_g.extend_from_slice(&parse_g1_points(supported_degree)?);

    // NOTE: used for hiding variant of KZG, not supported in Aztec's CRS.
    let powers_of_gamma_g = BTreeMap::new();
    // NOTE: not supported in Aztec's CRS.
    let neg_powers_of_h = BTreeMap::new();

    let h = G2Affine::generator();
    let [beta_h, _] = parse_g2_points()?;
    let prepared_h = h.into();
    let prepared_beta_h = beta_h.into();

    let pp: UniversalParams<Bn254> = UniversalParams {
        powers_of_g,
        powers_of_gamma_g,
        h,
        beta_h,
        neg_powers_of_h,
        prepared_h,
        prepared_beta_h,
    };
    Ok(pp)
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
        TRANSCRIPT_DIR, num_full_transcript
    ))?;
    g1_points.extend_from_slice(&parse_g1_points_from_file(&mut f, remainder_num_points)?);

    Ok(g1_points)
}

// Parse G1Affine points from CRS
// Concrete format spec:
// https://github.com/AztecProtocol/ignition-verification/blob/master/Transcript_spec.md#structure-of-a-transcript-file
#[allow(clippy::needless_range_loop)]
fn parse_g1_points_from_file(f: &mut File, num_points: usize) -> Result<Vec<G1Affine>> {
    let mut g1_points = Vec::new();

    if num_points > NUM_G1_PER_TRANSCRIPT {
        bail!("Internal Error, should not retrieve more than 5 million points per file");
    }

    for i in 0..num_points {
        // [X, Y]
        let mut bigints_repr = [[0u64; 4]; NUM_BIGINT_PER_G1];
        for j in 0..NUM_BIGINT_PER_G1 {
            for k in 0usize..4 {
                // size of G1 is 64
                // size of a bigint is 32 bytes.
                // size of u64 is 8 bytes.
                f.seek(SeekFrom::Start(
                    G1_STARTING_POS + i as u64 * 64 + j as u64 * 32 + k as u64 * 8,
                ))?;
                let mut buf = [0u8; 8];
                f.read_exact(&mut buf)?;
                bigints_repr[j][k] = u64::from_be_bytes(buf);
            }
        }

        let x = Fq::from_bigint(BigInteger256::new(bigints_repr[0]))
            .expect("Failed to parse G1 point's x-coordinate");
        let y = Fq::from_bigint(BigInteger256::new(bigints_repr[1]))
            .expect("Failed to parse G1 point's y-coordinate");

        let point = G1Affine::new(x, y); // point.is_on_curve() already checked during `new()`
        g1_points.push(point);
    }
    Ok(g1_points)
}

// Parse G2Affine points from CRS
// Concrete format spec:
// https://github.com/AztecProtocol/ignition-verification/blob/master/Transcript_spec.md#structure-of-a-transcript-file
// NOTE: the second G2 point is not used in CRS, but only for transcript
// verification purposes.
#[allow(clippy::needless_range_loop)]
fn parse_g2_points() -> Result<[G2Affine; NUM_G2]> {
    let mut g2_points = [G2Affine::default(); NUM_G2];

    // Parse 2 G2Affine points from CRS
    let mut f = File::open(format!("{}/transcript00.dat", TRANSCRIPT_DIR))?;
    for i in 0..NUM_G2 {
        // [x.c0, x.c1, y.c0, y.c1]
        let mut bigints_repr = [[0u64; 4]; NUM_BIGINT_PER_G2];

        for j in 0..NUM_BIGINT_PER_G2 {
            for k in 0usize..4 {
                // size of G1 is 64
                // size of G2 is 128 bytes (4 * 32).
                // size of a bigint is 32 bytes.
                // size of u64 is 8 bytes.
                f.seek(SeekFrom::Start(
                    G1_STARTING_POS
                        + NUM_G1_PER_TRANSCRIPT as u64 * 64
                        + i as u64 * 128
                        + j as u64 * 32
                        + k as u64 * 8,
                ))?;
                let mut buf = [0u8; 8];
                f.read_exact(&mut buf)?;
                bigints_repr[j][k] = u64::from_be_bytes(buf);
            }
        }
        let x_c0 = Fq::from_bigint(BigInteger256::new(bigints_repr[0]))
            .expect("Failed to parse G2 point's X.c0");
        let x_c1 = Fq::from_bigint(BigInteger256::new(bigints_repr[1]))
            .expect("Failed to parse G2 point's X.c1");
        let y_c0 = Fq::from_bigint(BigInteger256::new(bigints_repr[2]))
            .expect("Failed to parse G2 point's Y.c0");
        let y_c1 = Fq::from_bigint(BigInteger256::new(bigints_repr[3]))
            .expect("Failed to parse G2 point's Y.c1");
        let x = Fq2::new(x_c0, x_c1);
        let y = Fq2::new(y_c0, y_c1);

        let point = G2Affine::new(x, y);
        assert!(point.is_on_curve());
        g2_points[i] = point;
    }

    Ok(g2_points)
}

#[cfg(test)]
mod test {
    use super::*;
    use ark_ec::{pairing::Pairing, CurveGroup, VariableBaseMSM};
    use ark_ff::UniformRand;
    use ark_poly::{univariate::DensePolynomial, DenseUVPolynomial, Polynomial};
    use ark_poly_commit::{
        kzg10::{Powers, Proof, Randomness, VerifierKey, KZG10},
        PCRandomness,
    };
    use ark_std::ops::Div;

    // simplify from arkworks' poly-commit
    pub fn open<'c, E, P>(
        powers: &Powers<E>,
        p: &P,
        point: P::Point,
        rand: &Randomness<E::ScalarField, P>,
    ) -> Result<Proof<E>>
    where
        E: Pairing,
        P: DenseUVPolynomial<E::ScalarField, Point = E::ScalarField>,
        for<'a, 'b> &'a P: Div<&'b P, Output = P>,
    {
        // check_degree_is_too_large()
        let degree = p.degree();
        let num_coefficients = degree + 1;
        if num_coefficients > powers.size() {
            bail!("Too many coefficients");
        }

        assert_eq!(rand, &Randomness::empty());
        let (witness_poly, _hiding_witness_poly) =
            KZG10::<E, P>::compute_witness_polynomial(p, point, rand)?;

        // adapated from `open_with_witness_polynomial()`
        let (num_leading_zeros, witness_coeffs) =
            skip_leading_zeros_and_convert_to_bigints(&witness_poly);

        let w = <E::G1 as VariableBaseMSM>::msm_bigint(
            &powers.powers_of_g[num_leading_zeros..],
            &witness_coeffs,
        );
        Ok(Proof {
            w: w.into_affine(),
            random_v: None,
        })
    }

    /// Specializes the public parameters for a given maximum degree `d` for
    /// polynomials `d` should be less that `pp.max_degree()`.
    /// Modify from <https://github.com/arkworks-rs/poly-commit/blob/master/poly-commit/src/kzg10/mod.rs#L512>
    fn trim<E: Pairing>(
        pp: &UniversalParams<E>,
        mut supported_degree: usize,
    ) -> Result<(Powers<E>, VerifierKey<E>)> {
        if supported_degree == 1 {
            supported_degree += 1;
        }
        let powers_of_g = pp.powers_of_g[..=supported_degree].to_vec();

        let powers = Powers {
            powers_of_g: ark_std::borrow::Cow::Owned(powers_of_g),
            powers_of_gamma_g: ark_std::borrow::Cow::Owned(vec![]), // empty, not available
        };
        let vk = VerifierKey {
            g: pp.powers_of_g[0],
            gamma_g: E::G1Affine::default(), // dummy gamma_g, not available
            h: pp.h,
            beta_h: pp.beta_h,
            prepared_h: pp.prepared_h.clone(),
            prepared_beta_h: pp.prepared_beta_h.clone(),
        };
        Ok((powers, vk))
    }

    fn skip_leading_zeros_and_convert_to_bigints<F: PrimeField, P: DenseUVPolynomial<F>>(
        p: &P,
    ) -> (usize, Vec<F::BigInt>) {
        let mut num_leading_zeros = 0;
        while num_leading_zeros < p.coeffs().len() && p.coeffs()[num_leading_zeros].is_zero() {
            num_leading_zeros += 1;
        }
        let coeffs = ark_std::cfg_iter!(&p.coeffs()[num_leading_zeros..])
            .map(|s| s.into_bigint())
            .collect::<Vec<_>>();
        (num_leading_zeros, coeffs)
    }

    #[test]
    fn test_aztec_crs() -> Result<()> {
        let rng = &mut ark_std::test_rng();

        // adapted from https://github.com/arkworks-rs/poly-commit
        for _ in 0..10 {
            let mut degree = 0;
            while degree <= 1 {
                degree = usize::rand(rng) % 2usize.pow(16);
            }
            let srs = kzg10_setup(degree)?;
            let (ck, vk) = trim(&srs, degree)?;
            let p: DensePolynomial<ark_bn254::Fr> = DenseUVPolynomial::rand(degree, rng);
            let (comm, rand) = KZG10::commit(&ck, &p, None, None)?;
            let point = ark_bn254::Fr::rand(rng);
            let value = p.evaluate(&point);
            let proof = open(&ck, &p, point, &rand)?;
            assert!(
                KZG10::<ark_bn254::Bn254, DensePolynomial<ark_bn254::Fr>>::check(
                    &vk, &comm, point, value, &proof
                )?,
                "proof was incorrect for max_degree = {}, polynomial_degree = {}",
                degree,
                p.degree(),
            );
        }

        Ok(())
    }
}
