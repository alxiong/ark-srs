# crs

Use Common/Structured Reference String (CRS/SRS) from existing ceremonies with ease

**WARNING: This is work in progress, none of the code has been audited. The library is NOT ready for production.**

## Usage

```rust
use ark_bn::Bn254;
use ark_poly::univariate::DensePolynomial;

// simulated CRS (for test only)
let pp = KZG10::<Bn254, DensePolynomial<<Bn254 as PairingEngine>::Fr>>::setup(max_degree, false, &mut rng)?;

// now, use Aztec's CRS
let pp = use crs::aztec20::get_crs(max_degree, false)?;
```
