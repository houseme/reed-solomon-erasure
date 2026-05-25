#[cfg(feature = "std")]
use super::super::profile::{
    rust_neon_mul_slice_xor_schedule_split, rust_neon_mul_slice_xor_unroll,
    RUST_NEON_PROFILE_METRICS,
};

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_neon_mul_slice(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_neon_mul_slice_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
pub(crate) fn rust_neon_mul_slice_xor(c: u8, input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    if input.is_empty() {
        return;
    }

    unsafe { rust_neon_mul_slice_xor_impl(c, input, out) }
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "neon")]
unsafe fn rust_neon_mul_slice_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::aarch64::{
        uint8x16_t, uint8x16x4_t, vandq_u8, vdupq_n_u8, veorq_u8, vld1q_u8, vld1q_u8_x4,
        vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vst1q_u8_x4,
    };

    let low_tbl = unsafe { vld1q_u8(super::super::MUL_TABLE_LOW[c as usize].as_ptr()) };
    let high_tbl = unsafe { vld1q_u8(super::super::MUL_TABLE_HIGH[c as usize].as_ptr()) };
    let nibble_mask = vdupq_n_u8(0x0f);
    let bytes_done = input.len() & !15usize;
    let bytes_done_unrolled = input.len() & !63usize;
    #[cfg(feature = "std")]
    {
        let vector_64b_chunks = bytes_done_unrolled / 64;
        let vector_16b_chunks = (bytes_done - bytes_done_unrolled) / 16;
        let tail_bytes = input.len() - bytes_done;
        RUST_NEON_PROFILE_METRICS.record_call(
            false,
            input.len(),
            vector_64b_chunks,
            vector_16b_chunks,
            tail_bytes,
        );
    }

    let mut offset = 0usize;
    while offset < bytes_done_unrolled {
        let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
        let input0 = inputs.0;
        let input1 = inputs.1;
        let input2 = inputs.2;
        let input3 = inputs.3;

        let low0 = vandq_u8(input0, nibble_mask);
        let low1 = vandq_u8(input1, nibble_mask);
        let low2 = vandq_u8(input2, nibble_mask);
        let low3 = vandq_u8(input3, nibble_mask);

        let high0 = vshrq_n_u8::<4>(input0);
        let high1 = vshrq_n_u8::<4>(input1);
        let high2 = vshrq_n_u8::<4>(input2);
        let high3 = vshrq_n_u8::<4>(input3);

        let result0: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
        let result1: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
        let result2: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
        let result3: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));

        unsafe {
            vst1q_u8_x4(
                out.as_mut_ptr().add(offset),
                uint8x16x4_t(result0, result1, result2, result3),
            )
        };
        offset += 64;
    }

    while offset < bytes_done {
        let input_vec = unsafe { vld1q_u8(input.as_ptr().add(offset)) };
        let low = vandq_u8(input_vec, nibble_mask);
        let high = vshrq_n_u8::<4>(input_vec);
        let result: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low), vqtbl1q_u8(high_tbl, high));
        unsafe { vst1q_u8(out.as_mut_ptr().add(offset), result) };
        offset += 16;
    }

    super::super::scalar::mul_slice_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}

#[cfg(all(
    feature = "simd-accel",
    target_arch = "aarch64",
    not(target_env = "msvc"),
    not(any(target_os = "android", target_os = "ios"))
))]
#[target_feature(enable = "neon")]
unsafe fn rust_neon_mul_slice_xor_impl(c: u8, input: &[u8], out: &mut [u8]) {
    use core::arch::aarch64::{
        uint8x16_t, uint8x16x2_t, uint8x16x4_t, vandq_u8, vdupq_n_u8, veorq_u8, vld1q_u8,
        vld1q_u8_x2, vld1q_u8_x4, vqtbl1q_u8, vshrq_n_u8, vst1q_u8, vst1q_u8_x2, vst1q_u8_x4,
    };

    let low_tbl = unsafe { vld1q_u8(super::super::MUL_TABLE_LOW[c as usize].as_ptr()) };
    let high_tbl = unsafe { vld1q_u8(super::super::MUL_TABLE_HIGH[c as usize].as_ptr()) };
    let nibble_mask = vdupq_n_u8(0x0f);
    let unroll4 = {
        #[cfg(feature = "std")]
        {
            rust_neon_mul_slice_xor_unroll() != 2
        }
        #[cfg(not(feature = "std"))]
        {
            true
        }
    };
    let bytes_done = input.len() & !15usize;
    let bytes_done_unrolled = if unroll4 {
        input.len() & !63usize
    } else {
        input.len() & !31usize
    };
    #[cfg(feature = "std")]
    {
        let vector_64b_chunks = if unroll4 { bytes_done_unrolled / 64 } else { 0 };
        let vector_16b_chunks = if unroll4 {
            (bytes_done - bytes_done_unrolled) / 16
        } else {
            ((bytes_done_unrolled / 32) * 2) + ((bytes_done - bytes_done_unrolled) / 16)
        };
        let tail_bytes = input.len() - bytes_done;
        RUST_NEON_PROFILE_METRICS.record_call(
            true,
            input.len(),
            vector_64b_chunks,
            vector_16b_chunks,
            tail_bytes,
        );
    }

    let mut offset = 0usize;
    if unroll4 {
        let schedule_split = {
            #[cfg(feature = "std")]
            {
                rust_neon_mul_slice_xor_schedule_split()
            }
            #[cfg(not(feature = "std"))]
            {
                false
            }
        };
        if schedule_split {
            while offset < bytes_done_unrolled {
                let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
                let input0 = inputs.0;
                let input1 = inputs.1;
                let input2 = inputs.2;
                let input3 = inputs.3;

                let low0 = vandq_u8(input0, nibble_mask);
                let low1 = vandq_u8(input1, nibble_mask);
                let low2 = vandq_u8(input2, nibble_mask);
                let low3 = vandq_u8(input3, nibble_mask);

                let high0 = vshrq_n_u8::<4>(input0);
                let high1 = vshrq_n_u8::<4>(input1);
                let high2 = vshrq_n_u8::<4>(input2);
                let high3 = vshrq_n_u8::<4>(input3);

                let product0: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
                let product1: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
                let product2: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
                let product3: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));
                let outs: uint8x16x4_t = unsafe { vld1q_u8_x4(out.as_ptr().add(offset)) };
                unsafe {
                    vst1q_u8_x4(
                        out.as_mut_ptr().add(offset),
                        uint8x16x4_t(
                            veorq_u8(outs.0, product0),
                            veorq_u8(outs.1, product1),
                            veorq_u8(outs.2, product2),
                            veorq_u8(outs.3, product3),
                        ),
                    )
                };
                offset += 64;
            }
        } else {
            while offset < bytes_done_unrolled {
                let inputs: uint8x16x4_t = unsafe { vld1q_u8_x4(input.as_ptr().add(offset)) };
                let input0 = inputs.0;
                let input1 = inputs.1;
                let input2 = inputs.2;
                let input3 = inputs.3;

                let low0 = vandq_u8(input0, nibble_mask);
                let low1 = vandq_u8(input1, nibble_mask);
                let low2 = vandq_u8(input2, nibble_mask);
                let low3 = vandq_u8(input3, nibble_mask);

                let high0 = vshrq_n_u8::<4>(input0);
                let high1 = vshrq_n_u8::<4>(input1);
                let high2 = vshrq_n_u8::<4>(input2);
                let high3 = vshrq_n_u8::<4>(input3);

                let product0: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
                let product1: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));
                let product2: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low2), vqtbl1q_u8(high_tbl, high2));
                let product3: uint8x16_t =
                    veorq_u8(vqtbl1q_u8(low_tbl, low3), vqtbl1q_u8(high_tbl, high3));
                let outs: uint8x16x4_t = unsafe { vld1q_u8_x4(out.as_ptr().add(offset)) };
                unsafe {
                    vst1q_u8_x4(
                        out.as_mut_ptr().add(offset),
                        uint8x16x4_t(
                            veorq_u8(outs.0, product0),
                            veorq_u8(outs.1, product1),
                            veorq_u8(outs.2, product2),
                            veorq_u8(outs.3, product3),
                        ),
                    )
                };
                offset += 64;
            }
        }
    } else {
        while offset < bytes_done_unrolled {
            let inputs: uint8x16x2_t = unsafe { vld1q_u8_x2(input.as_ptr().add(offset)) };
            let input0 = inputs.0;
            let input1 = inputs.1;

            let low0 = vandq_u8(input0, nibble_mask);
            let low1 = vandq_u8(input1, nibble_mask);

            let high0 = vshrq_n_u8::<4>(input0);
            let high1 = vshrq_n_u8::<4>(input1);

            let product0: uint8x16_t =
                veorq_u8(vqtbl1q_u8(low_tbl, low0), vqtbl1q_u8(high_tbl, high0));
            let product1: uint8x16_t =
                veorq_u8(vqtbl1q_u8(low_tbl, low1), vqtbl1q_u8(high_tbl, high1));

            let outs: uint8x16x2_t = unsafe { vld1q_u8_x2(out.as_ptr().add(offset)) };
            unsafe {
                vst1q_u8_x2(
                    out.as_mut_ptr().add(offset),
                    uint8x16x2_t(veorq_u8(outs.0, product0), veorq_u8(outs.1, product1)),
                )
            };
            offset += 32;
        }
    }

    while offset < bytes_done {
        let input_vec = unsafe { vld1q_u8(input.as_ptr().add(offset)) };
        let low = vandq_u8(input_vec, nibble_mask);
        let high = vshrq_n_u8::<4>(input_vec);
        let product: uint8x16_t = veorq_u8(vqtbl1q_u8(low_tbl, low), vqtbl1q_u8(high_tbl, high));
        let out_vec = unsafe { vld1q_u8(out.as_ptr().add(offset)) };
        unsafe { vst1q_u8(out.as_mut_ptr().add(offset), veorq_u8(out_vec, product)) };
        offset += 16;
    }

    super::super::scalar::mul_slice_xor_pure_rust(c, &input[bytes_done..], &mut out[bytes_done..]);
}
