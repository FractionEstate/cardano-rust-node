/*!
**IMPORTANT:** The real KES implementation (CompactSum7KES with 128 periods)
is provided by the upstream `cardano-crypto-class` crate from
`cardano-base-rust`. This module re-exports a Rust wrapper that aligns with the
Haskell reference implementation and Cardano CLI key formats.

The wrapper exposes the following high-level types:

* [`KesSecretKey`] – wraps the upstream signing key and handles key evolution
* [`KesPublicKey`] – verification key compatible with Cardano CLI envelopes
* [`KesSignature`] – signed payload containing period metadata

Refer to the upstream project for the underlying cryptographic details. The
Rust wrapper mirrors the production configuration used by Cardano mainnet and
testnets (128 KES periods).
*/

mod implementation;

pub use implementation::*;
