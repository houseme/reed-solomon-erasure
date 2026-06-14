# aarch64 SIMD Release Checklist

## Pre-merge Verification (on aarch64 hardware)

### 1. Build Verification

```bash
cargo build --features 'std simd-accel'
cargo build --all --no-default-features
```

### 2. Correctness Tests

```bash
# All tests with SIMD acceleration
cargo test --release --features 'std simd-accel'

# NEON-specific tests
cargo test --release --features 'std simd-accel' -- rust_neon

# Scalar fallback
RSE_BACKEND_OVERRIDE=scalar RSE_STRICT_BACKEND_OVERRIDE=1 \
  cargo test --release --features 'std simd-accel'
```

### 3. Backend Override Verification

```bash
# NEON override
RSE_BACKEND_OVERRIDE=rust-neon RSE_STRICT_BACKEND_OVERRIDE=1 \
  cargo test --release --features 'std simd-accel' --test benchmark_smoke \
  benchmark_smoke_matrix_runs_and_exports_results -- --ignored --nocapture

# Scalar override
RSE_BACKEND_OVERRIDE=scalar RSE_STRICT_BACKEND_OVERRIDE=1 \
  cargo test --release --features 'std simd-accel' --test benchmark_smoke \
  benchmark_smoke_matrix_runs_and_exports_results -- --ignored --nocapture
```

### 4. Backend Consistency

```bash
bash scripts/run_aarch64_backend_smoke_matrix.sh
bash scripts/check_backend_consistency.sh
```

### 5. Cross-backend Conformance

```bash
cargo test --features 'std simd-accel' -- mul_slice --nocapture
```

This runs tests comparing NEON output against scalar, ensuring bitwise identical results.

## Smoke Test Matrix

| Backend | Test | Expected |
|---------|------|----------|
| auto | Full test suite | All pass |
| rust-neon | Override + smoke | All pass |
| scalar | Override + smoke | All pass |

## Known Issues

- `benchmark_smoke_metadata_tracks_aarch64_scalar_and_neon_overrides` fails due to `spin::Once` caching — this is a test design issue, not a backend issue. The backend is initialized once and cached; changing env vars after initialization has no effect.

## Future SVE Backend Admission Criteria

When adding a real SVE backend:

1. Scalar correctness: `rust-sve` output must match `scalar-rust` for all inputs
2. Override verification: `RSE_BACKEND_OVERRIDE=rust-sve` must work
3. Metadata consistency: `name`, `id`, `kind` fields must be correct
4. Dispatch priority: SVE must be preferred over NEON when available
5. Benchmark evidence: At least one SVE-capable machine showing throughput data
6. No regression: NEON and scalar paths must still pass all tests
