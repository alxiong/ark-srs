# crs

Use Common/Structured Reference String (CRS/SRS) from existing ceremonies with ease

**WARNING: This is work in progress, none of the code has been audited. The library is NOT ready for production.**

## Usage

```rust
use ark_bn254::Bn254;
use ark_poly::univariate::DensePolynomial;

// simulated CRS (for test only)
let pp = KZG10::<Bn254, DensePolynomial<<Bn254 as PairingEngine>::Fr>>::setup(max_degree, false, &mut rng)?;
// then pick the `supported_degree` <= `max_degree`
let (commit_key, ver_key) = trim(&pp, supported_degree)?;

// now, use Aztec's CRS
let (ck, vk) = use crs::aztec20::kzg10_setup(supported_degree)?;

// e.g. in Plonk, you would use:
// `domain_size` is the evaluation domain size for FFT.
// `2` is the degree for masking polynomial to achieve ZK.
let (ck, vk) = use crs::aztec20::kzg10_setup(domain_size + 2)?;
```
