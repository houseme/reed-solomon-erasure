//! VSX (Vector Scalar Extension) GF(2^8) multiply for ppc64le.
//!
//! Uses the same nibble-lookup algorithm as NEON/SSSE3:
//! `result = vec_perm(low_tbl, low_tbl, low_nibbles) ^ vec_perm(high_tbl, high_tbl, high_nibbles)`
//!
//! This module only compiles for `powerpc64` + `simd-vsx` (a nightly-only
//! target, since the VSX intrinsics are still unstable), so it is never parsed
//! by the stable x86/aarch64 CI.

// The manual `chunks_exact(64)` unrolled loop + `chunks_exact(16)` remainder
// deliberately mirrors the NEON/SSSE3 backends and keeps the unaligned pointer
// arithmetic explicit; rewriting it to `as_chunks` (as this nightly-only lint
// suggests) would obscure that structure in unsafe code that the x86/aarch64 CI
// cannot execute.
#![allow(clippy::chunks_exact_to_as_chunks)]

#[cfg(all(feature = "simd-vsx", target_arch = "powerpc64"))]
pub(crate) fn rust_vsx_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }
    if c == 0 {
        out.fill(0);
        return;
    }
    if c == 1 {
        out.copy_from_slice(input);
        return;
    }

    // SAFETY: reached only after the backend dispatcher confirmed VSX availability,
    // satisfying the callee's `#[target_feature(enable = "vsx")]` requirement.
    unsafe { rust_vsx_mul_slice_impl(c, input, out) }
}

#[cfg(all(feature = "simd-vsx", target_arch = "powerpc64"))]
pub(crate) fn rust_vsx_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }
    if c == 0 {
        return;
    }
    if c == 1 {
        for (i, o) in input.iter().zip(out.iter_mut()) {
            *o ^= *i;
        }
        return;
    }

    // SAFETY: reached only after the backend dispatcher confirmed VSX availability,
    // satisfying the callee's `#[target_feature(enable = "vsx")]` requirement.
    unsafe { rust_vsx_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(feature = "simd-vsx", target_arch = "powerpc64"))]
#[target_feature(enable = "vsx")]
unsafe fn rust_vsx_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::powerpc64::{
        vec_and, vec_perm, vec_splats, vec_sr, vec_xor, vector_unsigned_char,
    };

    // SAFETY: this fn is `#[target_feature(enable = "vsx")]` and is only reached
    // after the backend dispatcher confirmed VSX support, so the VSX intrinsics
    // are valid to execute here. Every pointer access uses `read_unaligned` /
    // `write_unaligned` and is bounded by the `chunks_exact(_mut)` iterators, and
    // the transmutes reinterpret 16-byte `MUL_TABLE_*` rows as equally sized VSX
    // vectors.
    unsafe {
        let low_tbl: vector_unsigned_char =
            core::mem::transmute(super::super::MUL_TABLE_LOW[c as usize]);
        let high_tbl: vector_unsigned_char =
            core::mem::transmute(super::super::MUL_TABLE_HIGH[c as usize]);
        let nibble_mask: vector_unsigned_char = vec_splats(0x0fu8);

        let bytes_done = input.len() & !15usize;
        let bytes_done_unrolled = input.len() & !63usize;

        let (simd_input, tail_input) = input.split_at(bytes_done);
        let (simd_out, tail_out) = out.split_at_mut(bytes_done);
        let (unrolled_input, remainder_input) = simd_input.split_at(bytes_done_unrolled);
        let (unrolled_out, remainder_out) = simd_out.split_at_mut(bytes_done_unrolled);

        // 4x-unrolled loop: 64 bytes per iteration.
        for (in_chunk, out_chunk) in unrolled_input
            .chunks_exact(64)
            .zip(unrolled_out.chunks_exact_mut(64))
        {
            let i0: vector_unsigned_char = core::ptr::read_unaligned(in_chunk.as_ptr().cast());
            let i1: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(16).cast());
            let i2: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(32).cast());
            let i3: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(48).cast());

            let r0 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i0, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i0, vec_splats(4u8))),
            );
            let r1 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i1, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i1, vec_splats(4u8))),
            );
            let r2 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i2, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i2, vec_splats(4u8))),
            );
            let r3 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i3, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i3, vec_splats(4u8))),
            );

            core::ptr::write_unaligned(out_chunk.as_mut_ptr().cast(), r0);
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(16).cast(), r1);
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(32).cast(), r2);
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(48).cast(), r3);
        }

        // 16-byte remainder loop.
        for (in_chunk, out_chunk) in remainder_input
            .chunks_exact(16)
            .zip(remainder_out.chunks_exact_mut(16))
        {
            let v: vector_unsigned_char = core::ptr::read_unaligned(in_chunk.as_ptr().cast());
            let result = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(v, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(v, vec_splats(4u8))),
            );
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().cast(), result);
        }

        // Scalar tail.
        super::super::scalar::mul_slice_pure_rust(c, tail_input, tail_out);
    }
}

#[cfg(all(feature = "simd-vsx", target_arch = "powerpc64"))]
#[target_feature(enable = "vsx")]
unsafe fn rust_vsx_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::powerpc64::{
        vec_and, vec_perm, vec_splats, vec_sr, vec_xor, vector_unsigned_char,
    };

    // SAFETY: this fn is `#[target_feature(enable = "vsx")]` and is only reached
    // after the backend dispatcher confirmed VSX support, so the VSX intrinsics
    // are valid to execute here. Every pointer access uses `read_unaligned` /
    // `write_unaligned` and is bounded by the `chunks_exact(_mut)` iterators, and
    // the transmutes reinterpret 16-byte `MUL_TABLE_*` rows as equally sized VSX
    // vectors.
    unsafe {
        let low_tbl: vector_unsigned_char =
            core::mem::transmute(super::super::MUL_TABLE_LOW[c as usize]);
        let high_tbl: vector_unsigned_char =
            core::mem::transmute(super::super::MUL_TABLE_HIGH[c as usize]);
        let nibble_mask: vector_unsigned_char = vec_splats(0x0fu8);

        let bytes_done = input.len() & !15usize;
        let bytes_done_unrolled = input.len() & !63usize;

        let (simd_input, tail_input) = input.split_at(bytes_done);
        let (simd_out, tail_out) = out.split_at_mut(bytes_done);
        let (unrolled_input, remainder_input) = simd_input.split_at(bytes_done_unrolled);
        let (unrolled_out, remainder_out) = simd_out.split_at_mut(bytes_done_unrolled);

        // 4x-unrolled loop: 64 bytes per iteration.
        for (in_chunk, out_chunk) in unrolled_input
            .chunks_exact(64)
            .zip(unrolled_out.chunks_exact_mut(64))
        {
            let i0: vector_unsigned_char = core::ptr::read_unaligned(in_chunk.as_ptr().cast());
            let i1: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(16).cast());
            let i2: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(32).cast());
            let i3: vector_unsigned_char =
                core::ptr::read_unaligned(in_chunk.as_ptr().add(48).cast());

            let p0 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i0, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i0, vec_splats(4u8))),
            );
            let p1 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i1, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i1, vec_splats(4u8))),
            );
            let p2 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i2, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i2, vec_splats(4u8))),
            );
            let p3 = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(i3, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(i3, vec_splats(4u8))),
            );

            let o0: vector_unsigned_char = core::ptr::read_unaligned(out_chunk.as_ptr().cast());
            let o1: vector_unsigned_char =
                core::ptr::read_unaligned(out_chunk.as_ptr().add(16).cast());
            let o2: vector_unsigned_char =
                core::ptr::read_unaligned(out_chunk.as_ptr().add(32).cast());
            let o3: vector_unsigned_char =
                core::ptr::read_unaligned(out_chunk.as_ptr().add(48).cast());

            core::ptr::write_unaligned(out_chunk.as_mut_ptr().cast(), vec_xor(o0, p0));
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(16).cast(), vec_xor(o1, p1));
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(32).cast(), vec_xor(o2, p2));
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().add(48).cast(), vec_xor(o3, p3));
        }

        // 16-byte remainder loop.
        for (in_chunk, out_chunk) in remainder_input
            .chunks_exact(16)
            .zip(remainder_out.chunks_exact_mut(16))
        {
            let v: vector_unsigned_char = core::ptr::read_unaligned(in_chunk.as_ptr().cast());
            let product = vec_xor(
                vec_perm(low_tbl, low_tbl, vec_and(v, nibble_mask)),
                vec_perm(high_tbl, high_tbl, vec_sr(v, vec_splats(4u8))),
            );
            let existing: vector_unsigned_char =
                core::ptr::read_unaligned(out_chunk.as_ptr().cast());
            core::ptr::write_unaligned(out_chunk.as_mut_ptr().cast(), vec_xor(existing, product));
        }

        // Scalar tail.
        super::super::scalar::mul_slice_xor_pure_rust(c, tail_input, tail_out);
    }
}

#[cfg(all(test, feature = "simd-vsx", target_arch = "powerpc64"))]
mod tests {
    use super::super::super::scalar;

    #[test]
    fn test_vsx_mul_slice_basic() {
        let input: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let mut out = vec![0u8; 256];
        super::rust_vsx_mul_slice(5, &input, &mut out);
        let mut expected = vec![0u8; 256];
        scalar::mul_slice_pure_rust(5, &input, &mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn test_vsx_mul_slice_xor_basic() {
        let input: Vec<u8> = (0..256).map(|i| i as u8).collect();
        let mut out = vec![0xAAu8; 256];
        let mut expected = out.clone();
        super::rust_vsx_mul_slice_xor(7, &input, &mut out);
        scalar::mul_slice_xor_pure_rust(7, &input, &mut expected);
        assert_eq!(out, expected);
    }

    #[test]
    fn test_vsx_mul_slice_zero() {
        let input: Vec<u8> = (0..128).map(|i| i as u8).collect();
        let mut out = vec![0xFFu8; 128];
        super::rust_vsx_mul_slice(0, &input, &mut out);
        assert!(out.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_vsx_mul_slice_one() {
        let input: Vec<u8> = (0..128).map(|i| i as u8).collect();
        let mut out = vec![0u8; 128];
        super::rust_vsx_mul_slice(1, &input, &mut out);
        assert_eq!(out, input);
    }

    #[test]
    fn test_vsx_mul_slice_reference_parity() {
        // Test multiple c values with a buffer larger than 64 bytes to exercise unrolled loop.
        let input: Vec<u8> = (0..200).map(|i| (i.wrapping_mul(37)) as u8).collect();
        for c in 2..=255u8 {
            let mut vsx_out = vec![0u8; 200];
            let mut ref_out = vec![0u8; 200];
            super::rust_vsx_mul_slice(c, &input, &mut vsx_out);
            scalar::mul_slice_pure_rust(c, &input, &mut ref_out);
            assert_eq!(vsx_out, ref_out, "mul_slice mismatch for c={c}");
        }
    }
}
