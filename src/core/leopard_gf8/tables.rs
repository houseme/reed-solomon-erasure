extern crate alloc;

use alloc::boxed::Box;

use super::ops::{add_mod8, fwht8, mul_log8};
use super::{BITWIDTH8, LeopardGf8Tables, MODULUS8, Mul8Lut, ORDER8, POLYNOMIAL8};

pub(super) fn build_tables8() -> LeopardGf8Tables {
    let (log_lut, exp_lut) = init_luts8();
    let log_lut_ref = &*log_lut;
    let exp_lut_ref = &*exp_lut;
    let (fft_skew, log_walsh) = init_fft_skew8(log_lut_ref, exp_lut_ref);
    let mul_luts = init_mul8_lut(log_lut_ref, exp_lut_ref);

    LeopardGf8Tables {
        fft_skew,
        log_walsh,
        log_lut,
        exp_lut,
        mul_luts,
    }
}

fn init_luts8() -> (Box<[u8; ORDER8]>, Box<[u8; ORDER8]>) {
    let cantor_basis = [1u8, 214, 152, 146, 86, 200, 88, 230];
    let mut exp_lut = Box::new([0u8; ORDER8]);
    let mut log_lut = Box::new([0u8; ORDER8]);

    let mut state = 1usize;
    for i in 0..MODULUS8 {
        exp_lut[state] = i as u8;
        state <<= 1;
        if state >= ORDER8 {
            state ^= POLYNOMIAL8;
        }
    }
    exp_lut[0] = MODULUS8 as u8;

    log_lut[0] = 0;
    for (i, basis) in cantor_basis.iter().copied().enumerate() {
        let width = 1usize << i;
        for j in 0..width {
            log_lut[j + width] = log_lut[j] ^ basis;
        }
    }

    for i in 0..ORDER8 {
        log_lut[i] = exp_lut[log_lut[i] as usize];
    }

    for i in 0..ORDER8 {
        exp_lut[log_lut[i] as usize] = i as u8;
    }
    exp_lut[MODULUS8] = exp_lut[0];

    (log_lut, exp_lut)
}

fn init_fft_skew8(
    log_lut: &[u8; ORDER8],
    exp_lut: &[u8; ORDER8],
) -> (Box<[u8; MODULUS8]>, Box<[u8; ORDER8]>) {
    let mut temp = [0u8; BITWIDTH8 - 1];
    for i in 1..BITWIDTH8 {
        temp[i - 1] = (1usize << i) as u8;
    }

    let mut fft_skew = Box::new([0u8; MODULUS8]);
    let mut log_walsh = Box::new([0u8; ORDER8]);

    for m in 0..(BITWIDTH8 - 1) {
        let step = 1usize << (m + 1);
        fft_skew[(1usize << m) - 1] = 0;

        for i in m..(BITWIDTH8 - 1) {
            let s = 1usize << (i + 1);
            let mut j = (1usize << m) - 1;
            while j < s {
                fft_skew[j + s] = fft_skew[j] ^ temp[i];
                j += step;
            }
        }

        temp[m] = (MODULUS8
            - mul_log8(temp[m], log_lut[(temp[m] ^ 1) as usize], log_lut, exp_lut) as usize)
            as u8;

        for i in (m + 1)..(BITWIDTH8 - 1) {
            let sum = add_mod8(log_lut[(temp[i] ^ 1) as usize], temp[m]);
            temp[i] = mul_log8(temp[i], sum, log_lut, exp_lut);
        }
    }

    for i in 0..MODULUS8 {
        fft_skew[i] = log_lut[fft_skew[i] as usize];
    }

    for i in 0..ORDER8 {
        log_walsh[i] = log_lut[i];
    }
    log_walsh[0] = 0;
    fwht8(&mut log_walsh);

    (fft_skew, log_walsh)
}

fn init_mul8_lut(log_lut: &[u8; ORDER8], exp_lut: &[u8; ORDER8]) -> Box<[Mul8Lut; ORDER8]> {
    let mut mul_luts = Box::new([Mul8Lut {
        value: [0u8; 256],
        low: [0u8; 16],
        high: [0u8; 16],
    }; ORDER8]);

    for log_m in 0..ORDER8 {
        let mut tmp = [0u8; 64];
        let mut nibble = 0usize;
        let mut shift = 0usize;
        while nibble < 4 {
            let start = nibble * 16;
            for x_nibble in 0..16usize {
                tmp[start + x_nibble] =
                    mul_log8((x_nibble << shift) as u8, log_m as u8, log_lut, exp_lut);
            }
            nibble += 1;
            shift += 4;
        }

        let lut = &mut mul_luts[log_m];
        for i in 0..256usize {
            lut.value[i] = tmp[i & 15] ^ tmp[(i >> 4) + 16];
        }

        // Pre-split nibble tables for SIMD nibble-lookup.
        // low[i] = value[i] (low nibble products)
        // high[i] = value[i * 16] (high nibble products)
        lut.low.copy_from_slice(&lut.value[..16]);
        for i in 0..16 {
            lut.high[i] = lut.value[i * 16];
        }
    }

    mul_luts
}
