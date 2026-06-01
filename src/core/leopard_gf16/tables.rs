extern crate alloc;

use alloc::boxed::Box;

use super::ops::{add_mod16, fwht16, gf16_mul};
use super::{LeopardGf16Tables, MODULUS16, ORDER16, POLYNOMIAL16};

pub(crate) fn build_tables16() -> LeopardGf16Tables {
    let (log_lut, exp_lut) = init_luts16();
    let log_lut_ref = &*log_lut;
    let exp_lut_ref = &*exp_lut;
    let (fft_skew, log_walsh) = init_fft_skew16(log_lut_ref, exp_lut_ref);

    LeopardGf16Tables {
        fft_skew,
        log_walsh,
        log_lut,
        exp_lut,
    }
}

fn init_luts16() -> (Box<[u16; ORDER16]>, Box<[u16; ORDER16 * 2]>) {
    let mut log_lut = Box::new([0u16; ORDER16]);
    let mut exp_lut = Box::new([0u16; ORDER16 * 2]);

    // Build exp table: exp[i] = primitive_element^i for i in 0..MODULUS16.
    // Use standard polynomial construction: start with 1, shift left, reduce by polynomial.
    let mut x: u32 = 1;
    for i in 0..MODULUS16 {
        log_lut[x as usize] = i as u16;
        exp_lut[i] = x as u16;
        x <<= 1;
        if x >= ORDER16 as u32 {
            x ^= POLYNOMIAL16;
        }
    }
    // Wraparound: exp[MODULUS16] = exp[0] so that exp[a + MODULUS16] = exp[a].
    exp_lut[MODULUS16] = exp_lut[0];

    // log[0] is unused (log of zero is undefined), leave as 0.

    (log_lut, exp_lut)
}

fn init_fft_skew16(
    log_lut: &[u16; ORDER16],
    exp_lut: &[u16; ORDER16 * 2],
) -> (Box<[u16; MODULUS16]>, Box<[u16; ORDER16]>) {
    // Port of init_fft_skew8 to GF16.
    let bitwidth = super::BITWIDTH16;
    let mut temp = [0u16; super::BITWIDTH16]; // BITWIDTH16 elements
    for i in 1..bitwidth {
        temp[i - 1] = 1u16 << i;
    }

    let mut fft_skew = Box::new([0u16; MODULUS16]);
    let mut log_walsh = Box::new([0u16; ORDER16]);

    for m in 0..(bitwidth - 1) {
        let step = 1usize << (m + 1);
        fft_skew[(1usize << m) - 1] = 0;

        for i in m..(bitwidth - 1) {
            let s = 1usize << (i + 1);
            let mut j = (1usize << m) - 1;
            while j < s {
                fft_skew[j + s] = fft_skew[j] ^ temp[i];
                j += step;
            }
        }

        let gf_prod = gf16_mul(temp[m], log_lut[(temp[m] ^ 1) as usize], log_lut, exp_lut);
        temp[m] = (MODULUS16 as u32 - log_lut[gf_prod as usize] as u32) as u16;

        for i in (m + 1)..(bitwidth - 1) {
            let sum = add_mod16(log_lut[(temp[i] ^ 1) as usize], temp[m]);
            temp[i] = gf16_mul(temp[i], sum, log_lut, exp_lut);
        }
    }

    for i in 0..MODULUS16 {
        fft_skew[i] = log_lut[fft_skew[i] as usize];
    }

    for i in 0..ORDER16 {
        log_walsh[i] = log_lut[i];
    }
    log_walsh[0] = 0;
    fwht16(&mut log_walsh);

    (fft_skew, log_walsh)
}
