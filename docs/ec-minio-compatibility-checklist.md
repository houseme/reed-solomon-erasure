# MinIO Compatibility Checklist

## Scope

This checklist distinguishes changes that are safe for classic MinIO-compatible shard generation from changes
that are either conditionally safe or known to alter output compatibility.

It covers compatibility at the shard-payload level. Full MinIO on-disk object compatibility also depends on
metadata, placement, part sizing, and checksum rules outside this crate.

## Safe Optimizations

These changes are safe because they only change how bytes are computed, not which bytes are produced.

### SIMD and backend execution changes

- runtime backend dispatch
- AVX2/AVX512/GFNI/NEON backend additions
- backend override controls for testing
- legacy C fallback cleanup

Condition:

- The backend must implement the same field operations as the classic path.

### Parallel scheduling and chunking

- parallel policy tuning
- chunk-size tuning
- small-output parallel fast paths
- backend-specific reconstruct policy tuning

Condition:

- Work splitting must not reorder algebra in a way that changes final shard bytes.

### Cache and planning improvements

- inversion cache changes
- decode-matrix cache sizing
- reconstruction planner refactors
- required-only reconstruct copy reduction

Condition:

- The selected valid/invalid shard indices and matrix rows must stay logically equivalent.

### Allocation and buffering improvements

- aligned allocation helpers
- reusable verify workspace improvements
- scratch-buffer reuse
- zero-copy internal borrowing where semantics remain the same

Condition:

- Padding rules and buffer initialization behavior must remain unchanged.

### New classic-path APIs that preserve output

- parity `update` API
- progressive `decode_idx` API
- additional verify/reconstruct workspace APIs

Condition:

- Output must be byte-identical to the existing classic encode/reconstruct path for equivalent inputs.

## Conditionally Safe Changes

These can remain MinIO-compatible, but only when constrained carefully.

### `split()` and `join()` behavior changes

Safe only if all of the following remain true:

- shard ordering is unchanged
- zero padding rules are unchanged
- parity shard count expectations are unchanged
- caller-visible output lengths are unchanged

Risk:

- A seemingly harmless split/join improvement can break interoperability if it changes padding semantics.

### Real `MatrixMode` support

Safe only for:

- the existing default classic matrix mode

Not automatically safe for:

- `Cauchy`
- `JerasureLike`
- `Custom`

Risk:

- The public enum can exist safely, but once callers choose a non-classic mode, output compatibility changes.

### `fast_one_parity`

Safe only if:

- it is mathematically equivalent to the classic matrix for one-parity configurations

Risk:

- if the one-parity mode changes matrix semantics rather than simply optimizing XOR execution, compatibility breaks

### Future alternative codec auto-selection

Safe only if:

- auto-selection never switches away from the classic codec family for MinIO-oriented configurations

Risk:

- transparent family switching is easy to miss and would create silent incompatibility

## Breaks Compatibility

These changes should be treated as incompatible with classic MinIO shard generation unless a separate compatibility
layer explicitly proves otherwise.

### Non-classic matrix families

- `Cauchy`
- `JerasureLike`
- `PAR1`-style behavior
- arbitrary `CustomMatrix`

Why:

- These change the coding matrix and therefore change parity output.

### Leopard family codecs

- Leopard GF8
- Leopard GF16

Why:

- They are alternative codec families with different constraints and compatibility expectations.
- They must be opt-in and documented as such.

### Silent change of default matrix mode

- changing default from classic Vandermonde-compatible behavior to any other mode

Why:

- Existing encoded objects would no longer match new outputs for the same input.

### Silent change of shard ordering or padding contract

- different shard order
- different final-shard zero-padding rules
- different split/join data layout

Why:

- Even with the same field arithmetic, byte layout changes invalidate interoperability.

## Full MinIO Compatibility Requires More Than This Crate

Even when shard payload generation is compatible, full MinIO object compatibility still requires matching:

- object-part sizing strategy
- erasure distribution metadata
- checksums and bitrot rules
- object metadata layout
- healing and placement assumptions

Therefore:

- classic-path compatibility in this crate is necessary
- but not sufficient for direct disk-format interchange with MinIO

## Default Project Rule

If the goal is MinIO-compatible shard generation, the project should keep the following defaults:

- classic GF(2^8) path
- classic Vandermonde-compatible matrix behavior
- unchanged shard order
- unchanged padding rules
- performance changes limited to execution strategy, caching, and allocation behavior

## Review Gate

When reviewing any EC patch, ask these questions:

1. Does this change the coding matrix?
2. Does this change shard ordering?
3. Does this change padding or split/join layout?
4. Does this switch codec family?
5. Does this only change execution strategy?

If answers 1-4 are all `no` and answer 5 is `yes`, the patch is usually safe for classic MinIO compatibility.
