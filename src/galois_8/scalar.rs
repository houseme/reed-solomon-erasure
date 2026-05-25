const PURE_RUST_UNROLL: isize = 4;

pub(crate) fn mul_slice_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &super::MUL_TABLE[c as usize];
    let mt_ptr: *const u8 = &mt[0];

    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    if len == 0 {
        return;
    }

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr = *mt_ptr.offset(*input_ptr as isize);
                *out_ptr.offset(1) = *mt_ptr.offset(*input_ptr.offset(1) as isize);
                *out_ptr.offset(2) = *mt_ptr.offset(*input_ptr.offset(2) as isize);
                *out_ptr.offset(3) = *mt_ptr.offset(*input_ptr.offset(3) as isize);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr = *mt_ptr.offset(*input_ptr as isize);

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] = mt[input[n] as usize]
     * }
     */
}

pub(crate) fn mul_slice_xor_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &super::MUL_TABLE[c as usize];
    let mt_ptr: *const u8 = &mt[0];

    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    if len == 0 {
        return;
    }

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr ^= *mt_ptr.offset(*input_ptr as isize);
                *out_ptr.offset(1) ^= *mt_ptr.offset(*input_ptr.offset(1) as isize);
                *out_ptr.offset(2) ^= *mt_ptr.offset(*input_ptr.offset(2) as isize);
                *out_ptr.offset(3) ^= *mt_ptr.offset(*input_ptr.offset(3) as isize);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr ^= *mt_ptr.offset(*input_ptr as isize);

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] ^= mt[input[n] as usize];
     * }
     */
}

#[cfg(test)]
pub(crate) fn slice_xor(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());

    let len: isize = input.len() as isize;
    if len == 0 {
        return;
    }

    let mut input_ptr: *const u8 = &input[0];
    let mut out_ptr: *mut u8 = &mut out[0];

    let mut n: isize = 0;
    unsafe {
        assert_eq!(4, PURE_RUST_UNROLL);
        if len > PURE_RUST_UNROLL {
            let len_minus_unroll = len - PURE_RUST_UNROLL;
            while n < len_minus_unroll {
                *out_ptr ^= *input_ptr;
                *out_ptr.offset(1) ^= *input_ptr.offset(1);
                *out_ptr.offset(2) ^= *input_ptr.offset(2);
                *out_ptr.offset(3) ^= *input_ptr.offset(3);

                input_ptr = input_ptr.offset(PURE_RUST_UNROLL);
                out_ptr = out_ptr.offset(PURE_RUST_UNROLL);
                n += PURE_RUST_UNROLL;
            }
        }
        while n < len {
            *out_ptr ^= *input_ptr;

            input_ptr = input_ptr.offset(1);
            out_ptr = out_ptr.offset(1);
            n += 1;
        }
    }
    /* for n in 0..input.len() {
     *   out[n] ^= input[n]
     * }
     */
}
