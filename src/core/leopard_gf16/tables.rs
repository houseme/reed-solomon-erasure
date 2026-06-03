extern crate alloc;

use alloc::boxed::Box;

use super::ops::{add_mod16, fwht16, mul_log16};
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
    // Cantor basis for GF(2^16), matching the Go library (klauspost/reedsolomon).
    const CANTOR_BASIS16: [u16; super::BITWIDTH16] = [
        0x0001, 0xACCA, 0x3C0E, 0x163E,
        0xC582, 0xED2E, 0x914C, 0x4012,
        0x6C98, 0x10D8, 0x6A72, 0xB900,
        0xFDB8, 0xFB34, 0xFF38, 0x991E,
    ];

    let mut log_lut = Box::new([0u16; ORDER16]);
    let mut exp_lut = Box::new([0u16; ORDER16 * 2]);

    // Phase 1: Build exp_lut[state] = exponent via LFSR.
    // This maps each nonzero field element (state) to its LFSR step index.
    let mut state: usize = 1;
    for i in 0..MODULUS16 {
        exp_lut[state] = i as u16;
        state <<= 1;
        if state >= ORDER16 {
            state ^= POLYNOMIAL16 as usize;
        }
    }
    exp_lut[0] = MODULUS16 as u16;

    // Phase 2: Build log_lut using Cantor basis.
    // First, fill log_lut with Cantor-encoded values.
    log_lut[0] = 0;
    for (i, basis) in CANTOR_BASIS16.iter().copied().enumerate() {
        let width = 1usize << i;
        for j in 0..width {
            log_lut[j + width] = log_lut[j] ^ basis;
        }
    }

    // Translate Cantor-encoded values through exp_lut to get actual log values.
    for i in 0..ORDER16 {
        log_lut[i] = exp_lut[log_lut[i] as usize];
    }

    // Phase 3: Invert — exp_lut[log_lut[i]] = i, so exp_lut maps log → field element.
    for i in 0..ORDER16 {
        exp_lut[log_lut[i] as usize] = i as u16;
    }
    exp_lut[MODULUS16] = exp_lut[0];

    (log_lut, exp_lut)
}

fn init_fft_skew16(
    log_lut: &[u16; ORDER16],
    exp_lut: &[u16; ORDER16 * 2],
) -> (Box<[u16; MODULUS16]>, Box<[u16; ORDER16]>) {
    // Port of init_fft_skew8 to GF16.
    let bitwidth = super::BITWIDTH16;
    let mut temp = [0u16; super::BITWIDTH16 - 1]; // BITWIDTH16 - 1 elements, matching Go
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

        let gf_prod = mul_log16(temp[m], log_lut[(temp[m] ^ 1) as usize], log_lut, exp_lut);
        temp[m] = (MODULUS16 as u32 - log_lut[gf_prod as usize] as u32) as u16;

        for i in (m + 1)..(bitwidth - 1) {
            let sum = add_mod16(log_lut[(temp[i] ^ 1) as usize], temp[m]);
            temp[i] = mul_log16(temp[i], sum, log_lut, exp_lut);
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
