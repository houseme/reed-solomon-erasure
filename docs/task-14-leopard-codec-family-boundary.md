# Task 14: Leopard Codec Family Boundary

## 1. Goal

Define how Leopard GF8/GF16 support should be introduced as an explicit alternative codec family without polluting
the classic MinIO-compatible path.

This document started as a boundary-first design note. It now also records the first implementation slice:

- explicit `CodecFamily`
- classic default preserved
- `LeopardGF8` / `LeopardGF16` exposed as opt-in prototype skeletons
- `LeopardGF8` is now constructible as an internal family state with setup metadata
- `LeopardGF8` now has an explicit prototype encode path
- `LeopardGF8` specialized `encode_opt(...)` route now calls into the new `core::leopard_gf8` module
- no Leopard verify/reconstruct path enabled yet

## 2. Why This Task Exists

The largest remaining upstream feature gap is not another small classic-path micro-optimization. It is the absence of
an algorithm family that scales better for high shard counts.

However, introducing Leopard casually would create two serious risks:

1. silent output compatibility breaks
2. accidental mixing of high-shard-count and MinIO-compatible goals

This task defines the guardrails before code is written.

## 3. Current Situation

Current mainline focus:

- classic GF(2^8)
- MinIO-oriented compatibility concerns
- erasure sets that generally live well below high-shard-count Leopard motivation

Therefore Leopard should be treated as:

- valuable
- future-facing
- explicitly opt-in
- clearly separated from classic mode

## 4. Scope

## 4.1 In scope

- codec-family API design
- compatibility documentation
- benchmark isolation plan
- rollout and defaulting rules

## 4.2 Out of scope

- full Leopard implementation in this task
- switching any default behavior

## 4.3 Current implementation status

Implemented in this slice:

- `CodecFamily::{Classic, LeopardGF8, LeopardGF16}`
- `CodecOptions::codec_family`
- `ReedSolomon::codec_family()`
- constructor-time family validation boundary
- explicit `UnsupportedLeopardPrototype` / `UnsupportedCodecFamily` errors
- constructible `LeopardGF8` internal family state
- `ReedSolomon::leopard_setup_matrix_shape()`
- explicit `LeopardGF8` family-specific encode routing
- initial `core::leopard_gf8` algorithm scaffold with LUT/init and encode-driver wiring
- dedicated Leopard setup benchmark artifact path
- dedicated Leopard encode benchmark artifact path

Not implemented yet:

- Leopard reconstruct path
- Leopard-specific benchmark track
  - beyond the first setup/encode artifact slice

## 5. Recommended Public Design

Introduce an explicit codec-family concept instead of hiding Leopard behind unrelated flags.

Suggested shape:

```rust
pub enum CodecFamily {
    Classic,
    LeopardGF8,
    LeopardGF16,
}
```

Then fold this into options:

```rust
pub struct CodecOptions {
    ...
    pub codec_family: CodecFamily,
}
```

Status:

- implemented
- defaults to `CodecFamily::Classic`

Do not use:

- hidden auto-switching in the default constructor
- backend override environment variables to select codec family

## 6. Defaulting Rules

The default constructor must continue to mean:

- classic GF(2^8)
- classic-compatible matrix behavior

Any Leopard family use must be explicit.

Status:

- preserved in code
- covered by constructor tests

## 7. Compatibility Rules

Leopard family introduction must follow these rules:

1. never replace classic path implicitly
2. never advertise Leopard output as classic-compatible unless proven
3. document all shard-size constraints and family-specific limitations
4. mark all benchmarks separately from classic-path ledgers

## 8. Implementation Boundary Plan

## 8.1 Code organization

Recommended organization:

- keep classic path under existing `src/core.rs` / `src/galois_8/*`
- add new family modules under clear separate paths
- avoid interleaving family-specific branching into every classic hot-path helper

## 8.2 API routing

Construction should select a family-specific implementation boundary early.

Avoid:

- deeply nested `if family == ...` checks inside inner loops

Prefer:

- family-specific codec object or internal dispatch object chosen at construction

Current implementation choice:

- family selection now happens at construction time
- `LeopardGF8` now constructs a dedicated internal family state instead of being rejected immediately
- `encode(...)` / `encode_sep(...)` / `encode_opt(...)` now route through explicit LeopardGF8 family state
- `encode_opt(...)` on the `galois_8` specialized path is now wired through `core::leopard_gf8`
- verify/reconstruct/update/decode_idx still reject non-classic execution with explicit errors
- classic hot paths are still untouched by inner-loop family branching

## 8.3 Feature gating

If implementation size is large, consider a cargo feature for Leopard work so the classic crate remains stable while
the family matures.

## 9. Benchmark and Validation Plan

Leopard must use a dedicated benchmark track.

Do not mix its results into classic smoke regressions.

Recommended benchmark groups:

- classic 16+16, 32+32, 64+64, 128+128
- Leopard GF8 same shapes
- Leopard GF16 high-count shapes only if/when supported

For each:

- encode throughput
- reconstruct all
- reconstruct one/partial
- matrix/setup overhead

## 10. Acceptance Criteria for Future Implementation

Before a Leopard implementation is considered production-ready:

1. family choice is explicit
2. docs clearly separate compatibility boundaries
3. benchmarks show where Leopard actually wins
4. classic-path regressions remain isolated from Leopard experiments

## 11. Risks

### R1. Silent compatibility drift

Mitigation:

- explicit family enum
- classic default preserved

### R2. Documentation ambiguity

Mitigation:

- family-specific README/docs section
- benchmark reports separated by family

### R3. Overloading current options model

Mitigation:

- keep codec family as a first-class concept, not a side flag

## 12. Rollout Guidance

Suggested pre-implementation PR:

- `task14: define leopard codec family boundary`

Suggested future implementation split:

1. family API and docs
2. Leopard GF8 prototype
3. benchmark/reporting
4. Leopard GF16 evaluation if warranted

## 13. Validation completed in this slice

The current prototype-boundary slice is validated by:

```bash
cargo check --tests
cargo test test_codec_options_default_matches_new -- --nocapture
cargo test test_codec_options_default_uses_classic_family -- --nocapture
cargo test test_codec_options_accepts_explicit_classic_family -- --nocapture
cargo test test_leopard_gf8_prototype_is_explicit_but_not_executed_yet -- --nocapture
cargo test test_leopard_gf16_prototype_is_explicit_but_not_executed_yet -- --nocapture
cargo test test_leopard_gf8_is_rejected_for_galois_16_field -- --nocapture
cargo test test_leopard_custom_matrix_path_is_rejected_for_now -- --nocapture
cargo test test_leopard_gf8_encode_opt_populates_parity -- --nocapture
cargo test benchmark_leopard_setup_32x16_1m_exports_results -- --nocapture
cargo test benchmark_leopard_setup_64x32_1m_exports_results -- --nocapture
cargo test benchmark_leopard_setup_64x32_4m_exports_results -- --nocapture
cargo test benchmark_leopard_encode_64x32_1m_exports_results -- --nocapture
cargo test benchmark_leopard_encode_64x32_4m_exports_results -- --nocapture
```

## 13.1 First dedicated Leopard benchmark artifact

The first isolated Leopard benchmark track now exists and is intentionally separate from classic regression ledgers.

Current artifact:

- `target/benchmark-smoke/leopard-setup-32x16_1m.csv`
- `target/benchmark-smoke/leopard-setup-32x16_1m.json`
- `target/benchmark-smoke/leopard-setup-64x32_1m.csv`
- `target/benchmark-smoke/leopard-setup-64x32_4m.csv`
- `target/benchmark-smoke/leopard-encode-64x32_1m.csv`
- `target/benchmark-smoke/leopard-encode-64x32_4m.csv`

Current recorded result:

- case: `32x16_1m`
- operation: `leopard_setup`
- throughput: `6286.8629 MB/s`
- `ns_per_iter`: `5089979.00`
- setup shape: `48 x 32`

- case: `64x32_1m`
  - operation: `leopard_setup`
  - throughput: `1874.6247 MB/s`
  - `ns_per_iter`: `34140166.50`
  - setup shape: `96 x 64`

- case: `64x32_4m`
  - operation: `leopard_setup`
  - throughput: `7526.3957 MB/s`
  - `ns_per_iter`: `34013625.00`
  - setup shape: `96 x 64`

- case: `64x32_1m`
  - operation: `leopard_encode`
  - previous prototype-route baseline: `5.7160 MB/s`
  - current specialized-kernel-route reading: `5.5964 MB/s`
  - after the first pure-Rust butterfly/mul/xor helper pass: `10.2512 MB/s`
  - after removing inner-loop temporary allocations: `13.0268 MB/s`
  - after reverting `zero` reuse on the main path: `15.1617 MB/s`
  - current `ns_per_iter`: `4221155354.00`

- case: `64x32_4m`
  - operation: `leopard_encode`
  - previous prototype-route baseline: `5.7897 MB/s`
  - after the pure-Rust helper pass and allocation cleanup: `12.8469 MB/s`
  - after reverting `zero` reuse on the main path: `15.1377 MB/s`
  - current `ns_per_iter`: `16911405041.50`

## 14. Next implementation step

The next practical slice should be:

1. keep `CodecFamily::Classic` as the default and fully stable
2. keep growing the dedicated Leopard GF8 internal module instead of branching inside classic inner loops
3. replace top-level `UnsupportedLeopardPrototype` execution guards one API at a time, next starting with Leopard verify/reconstruct
4. expand the separate Leopard benchmark track before any claim of default-worthy behavior
5. treat current Leopard encode numbers as prototype-only until a real algorithmic path exists
6. continue filling butterfly/mul/xor helpers inside `core::leopard_gf8` before broadening route coverage
7. the next concrete goal is to keep improving the `64x32_1m` reading from the current `13.0268 MB/s` toward a level where it is meaningful to compare against classic paths

Recent A/B note:

- `64x32_1m` `leopard-encode-ab`
  - `baseline`: `16.0869 MB/s`
  - `reuse_zero_only`: `15.6621 MB/s`

Current conclusion:

- `zero` buffer reuse is not a stable win in the current pure-Rust kernel
- reverting that optimization on the mainline recovered the standard path to `15.1617 MB/s` at `64x32_1m`
- the next implementation pass should keep `zero` reuse out of the mainline before pursuing further helper tuning

Latest mainline confirmation after restoring the stable chunk-zeroing behavior:

- `64x32_1m`: `15.0824 MB/s`
- `64x32_4m`: `15.2183 MB/s`

Current working baseline:

- treat the `15 MB/s` band as the stable mainline for the current pure-Rust `LeopardGF8` encode path
- future A/Bs should target smaller hotspots, such as `xor_dest_offset` handling, rather than coarse full-chunk zeroing

Updated mainline after restructuring later-group work-buffer flow:

- `64x32_1m`: `15.5209 MB/s`
- `64x32_4m`: `15.7723 MB/s`

Current interpretation:

- the later-group buffer restructuring is worth keeping
- it improves the mainline more than the smaller `zero`/`xor_clone` micro-knobs did

Latest mainline after reverting the regressive zero-source `fill(0)` substitution:

- `64x32_1m`: `15.6166 MB/s`
- `64x32_4m`: `15.8417 MB/s`

Current interpretation:

- the zero-source `fill(0)` substitution should stay out of the mainline
- the later-group work-buffer restructuring remains the better coarse-grained optimization

Follow-up A/B on `64x32_1m`:

- `baseline`: `15.4498 MB/s`
- `reuse_zero_only`: `15.4406 MB/s`
- `xor_clone_only`: `15.3955 MB/s`

Updated interpretation:

- neither `reuse_zero_only` nor `xor_clone_only` is currently a compelling optimization to keep
- the next round should look beyond these two knobs and find a different local hotspot in `ifft_dit_encoder8(...)`

Latest mainline after tightening full/remainder real-data load handling:

- `64x32_1m`: `15.6608 MB/s`
- `64x32_4m`: `16.0810 MB/s`

Current interpretation:

- the real data-shard load path is a stronger hotspot than the earlier `zero` and `xor_clone` micro-knobs
- this round is worth keeping in the mainline

Latest mainline after reverting the weaker later-layer call-organization tweak:

- `64x32_1m`: `16.1089 MB/s`
- `64x32_4m`: `15.8374 MB/s`

Current interpretation:

- the load-path improvements stay
- the `ifft_dit4_range(...)`-style call-organization change should stay out
- this is the current best stable mainline for the pure-Rust `LeopardGF8` encode path
