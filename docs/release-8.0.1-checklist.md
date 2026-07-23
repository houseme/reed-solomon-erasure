# 8.0.1 Release Checklist

This patch release ships the merged generated-AVX2 dispatch guard from PR #29 without changing the public API or encoded data format.

## Version Freeze

- `Cargo.toml` package and workspace dependency versions are `8.0.1`.
- `Cargo.lock` root package version is `8.0.1`.
- README and README_CN installation examples use `8.0.1`.
- `CHANGELOG.md` contains the dated 8.0.1 entry.
- The release branch has a clean worktree before tagging.

## Required Validation

```bash
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --exclude rustfs-erasure-codec-fuzz --features simd-accel
cargo test --no-default-features --features simd-avx2 --lib
cargo package --allow-dirty
```

The merged PR #29 CI is green across Linux stable/beta/nightly, Windows Native and GNU targets, macOS ARM64, Linux ARM64, ppc64le VSX, ASan, cargo-audit, and backend-override regression jobs. Rust 1.96 checks for `std simd-accel` and no_std `simd-avx2` passed on Linux x86-64. The no-AVX2 library validation passed; downstream RustFS system acceptance remains separately tracked.

## Downstream Acceptance Boundary

rustfs/backlog#1453 and rustfs/rustfs#5076 track the remaining real 6-drive `EC:2` multipart matrix on the reporter's no-AVX2-class hardware. Do not describe this downstream E2E as completed until its object checksums, process health, and multipart evidence are recorded there.

## Tag And Publish

After this release PR is merged and the required validation is green:

```bash
git switch main
git pull --ff-only
git tag -a 8.0.1 -m "release: 8.0.1"
git push origin 8.0.1
```

The tag triggers the repository release-preflight and crates.io publish workflows. Create the GitHub release only after those jobs succeed.
