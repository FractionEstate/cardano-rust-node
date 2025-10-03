# Shared Test Utilities

The `tests/common` module hosts reusable helpers for integration tests. These utilities keep fixture data, helper routines, and validation logic in a single place so individual test suites can focus on behavior instead of boilerplate setup.

Current contents:

- `vrf.rs`: Accessors for the official cardano-base Praos VRF golden vectors along with basic sanity checks to guard against accidental regressions in byte sizes.

When adding a new helper module, be sure to:

1. Document its purpose here.
2. Expose it from `mod.rs` so that every integration test can import it via the shared `common` module path.
3. Include targeted unit tests inside the helper module to ensure fixtures stay in sync with protocol constants.
