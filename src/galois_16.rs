//! GF(2^16) implementation.
//!
//! More accurately, this is a `GF((2^8)^2)` implementation which builds an extension
//! field of `GF(2^8)`, as defined in the `galois_8` module.

use crate::galois_8;
use core::ops::{Add, Div, Mul, Sub};

// the irreducible polynomial used as a modulus for the field.
// print R.irreducible_element(2,algorithm="first_lexicographic" )
// x^2 + a*x + a^7
//
// hopefully it is a fast polynomial
const EXT_POLY: [u8; 3] = [1, 2, 128];

/// The field GF(2^16).
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Field;

impl crate::Field for Field {
    const ORDER: usize = 65536;

    type Elem = [u8; 2];

    fn add(a: [u8; 2], b: [u8; 2]) -> [u8; 2] {
        (Element(a) + Element(b)).0
    }

    fn mul(a: [u8; 2], b: [u8; 2]) -> [u8; 2] {
        (Element(a) * Element(b)).0
    }

    fn div(a: [u8; 2], b: [u8; 2]) -> [u8; 2] {
        (Element(a) / Element(b)).0
    }

    fn exp(elem: [u8; 2], n: usize) -> [u8; 2] {
        Element(elem).exp(n).0
    }

    fn zero() -> [u8; 2] {
        [0; 2]
    }

    fn one() -> [u8; 2] {
        [0, 1]
    }

    fn nth_internal(n: usize) -> [u8; 2] {
        [(n >> 8) as u8, n as u8]
    }

    fn mul_slice(elem: [u8; 2], input: &[[u8; 2]], out: &mut [[u8; 2]]) {
        gf16_mul_slice(elem, input, out, false);
    }

    fn mul_slice_add(elem: [u8; 2], input: &[[u8; 2]], out: &mut [[u8; 2]]) {
        gf16_mul_slice(elem, input, out, true);
    }
}

/// SIMD-accelerated `out[i] = elem * input[i]` (or `out[i] ^= elem * input[i]`
/// when `accumulate`) over the `GF((2^8)^2)` tower field.
///
/// A GF(2^16) element `ax·X + ac` multiplied by the fixed `elem = cx·X + cc`
/// reduces (via `X² = 2·X + 128`, from [`EXT_POLY`]) to two GF(2^8) output
/// planes, each a linear combination of the `ax`/`ac` input byte planes:
///
/// * `out_x = A·ax ^ B·ac`, `out_c = C·ax ^ D·ac`
/// * `A = cc ^ 2·cx`, `B = cx`, `C = 128·cx`, `D = cc` (all in GF(2^8)).
///
/// Each `·` is a fixed-multiplier GF(2^8) slice multiply, so this reuses the
/// SIMD-accelerated [`galois_8::mul_slice`]/[`galois_8::mul_slice_xor`] backends
/// (ssse3/avx2/gfni/avx512/neon/vsx with runtime dispatch). Byte planes are
/// de-/re-interleaved in fixed stack chunks to stay `no_std` and allocation
/// free; the whole path is byte-wise GF(2^8), so it is endian-agnostic.
fn gf16_mul_slice(elem: [u8; 2], input: &[[u8; 2]], out: &mut [[u8; 2]], accumulate: bool) {
    assert_eq!(input.len(), out.len());

    let cx = elem[0];
    let cc = elem[1];
    let coef_a = cc ^ galois_8::mul(2, cx);
    let coef_b = cx;
    let coef_c = galois_8::mul(128, cx);
    let coef_d = cc;

    const CHUNK: usize = 1024;
    let mut ax = [0u8; CHUNK];
    let mut ac = [0u8; CHUNK];
    let mut ox = [0u8; CHUNK];
    let mut oc = [0u8; CHUNK];

    let mut offset = 0;
    while offset < input.len() {
        let n = core::cmp::min(CHUNK, input.len() - offset);
        let in_chunk = &input[offset..offset + n];
        let out_chunk = &mut out[offset..offset + n];

        for (i, e) in in_chunk.iter().enumerate() {
            ax[i] = e[0];
            ac[i] = e[1];
        }

        if accumulate {
            for (i, e) in out_chunk.iter().enumerate() {
                ox[i] = e[0];
                oc[i] = e[1];
            }
            galois_8::mul_slice_xor(coef_a, &ax[..n], &mut ox[..n]);
            galois_8::mul_slice_xor(coef_b, &ac[..n], &mut ox[..n]);
            galois_8::mul_slice_xor(coef_c, &ax[..n], &mut oc[..n]);
            galois_8::mul_slice_xor(coef_d, &ac[..n], &mut oc[..n]);
        } else {
            galois_8::mul_slice(coef_a, &ax[..n], &mut ox[..n]);
            galois_8::mul_slice_xor(coef_b, &ac[..n], &mut ox[..n]);
            galois_8::mul_slice(coef_c, &ax[..n], &mut oc[..n]);
            galois_8::mul_slice_xor(coef_d, &ac[..n], &mut oc[..n]);
        }

        for (i, e) in out_chunk.iter_mut().enumerate() {
            e[0] = ox[i];
            e[1] = oc[i];
        }
        offset += n;
    }
}

/// Type alias of ReedSolomon over GF(2^8).
pub type ReedSolomon = crate::ReedSolomon<Field>;

/// Type alias of ShardByShard over GF(2^8).
pub type ShardByShard<'a> = crate::ShardByShard<'a, Field>;

/// An element of `GF(2^16)`.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Element(pub [u8; 2]);

impl Element {
    // Create the zero element.
    fn zero() -> Self {
        Element([0, 0])
    }

    // A constant element evaluating to `n`.
    fn constant(n: u8) -> Element {
        Element([0, n])
    }

    // Whether this is the zero element.
    fn is_zero(&self) -> bool {
        self.0 == [0; 2]
    }

    fn exp(mut self, n: usize) -> Element {
        if n == 0 {
            Element::constant(1)
        } else if self == Element::zero() {
            Element::zero()
        } else {
            let x = self;
            for _ in 1..n {
                self = self * x;
            }

            self
        }
    }

    // reduces from some polynomial with degree <= 2.
    #[inline]
    fn reduce_from(mut x: [u8; 3]) -> Self {
        if x[0] != 0 {
            // divide x by EXT_POLY and use remainder.
            // i = 0 here.
            // c*x^(i+j)  = a*x^i*b*x^j
            x[1] ^= galois_8::mul(EXT_POLY[1], x[0]);
            x[2] ^= galois_8::mul(EXT_POLY[2], x[0]);
        }

        Element([x[1], x[2]])
    }

    fn degree(&self) -> usize {
        if self.0[0] != 0 { 1 } else { 0 }
    }
}

impl From<[u8; 2]> for Element {
    fn from(c: [u8; 2]) -> Self {
        Element(c)
    }
}

impl Default for Element {
    fn default() -> Self {
        Element::zero()
    }
}

impl Add for Element {
    type Output = Element;

    fn add(self, other: Self) -> Element {
        Element([self.0[0] ^ other.0[0], self.0[1] ^ other.0[1]])
    }
}

impl Sub for Element {
    type Output = Element;

    fn sub(self, other: Self) -> Element {
        self.add(other)
    }
}

impl Mul for Element {
    type Output = Element;

    fn mul(self, rhs: Self) -> Element {
        // FOIL; our elements are linear at most, with two coefficients
        let out: [u8; 3] = [
            galois_8::mul(self.0[0], rhs.0[0]),
            galois_8::add(
                galois_8::mul(self.0[1], rhs.0[0]),
                galois_8::mul(self.0[0], rhs.0[1]),
            ),
            galois_8::mul(self.0[1], rhs.0[1]),
        ];

        Element::reduce_from(out)
    }
}

impl Mul<u8> for Element {
    type Output = Element;

    fn mul(self, rhs: u8) -> Element {
        Element([galois_8::mul(rhs, self.0[0]), galois_8::mul(rhs, self.0[1])])
    }
}

impl Div for Element {
    type Output = Element;

    #[allow(clippy::suspicious_arithmetic_impl)]
    fn div(self, rhs: Self) -> Element {
        self * rhs.inverse()
    }
}

// helpers for division.

#[derive(Debug)]
enum EgcdRhs {
    Element(Element),
    ExtPoly,
}

impl Element {
    // compute extended euclidean algorithm against an element of self,
    // where the GCD is known to be constant.
    fn const_egcd(self, rhs: EgcdRhs) -> (u8, Element, Element) {
        if self.is_zero() {
            let rhs = match rhs {
                EgcdRhs::Element(elem) => elem,
                EgcdRhs::ExtPoly => {
                    debug_assert!(false, "const_egcd invoked with divisible");
                    Element::constant(1)
                }
            };
            (rhs.0[1], Element::constant(0), Element::constant(1))
        } else {
            let (cur_quotient, cur_remainder) = match rhs {
                EgcdRhs::Element(rhs) => rhs.polynom_div(self),
                EgcdRhs::ExtPoly => Element::div_ext_by(self),
            };

            // GCD is constant because EXT_POLY is irreducible
            let (g, x, y) = cur_remainder.const_egcd(EgcdRhs::Element(self));
            (g, y + (cur_quotient * x), x)
        }
    }

    // divide EXT_POLY by self.
    fn div_ext_by(rhs: Self) -> (Element, Element) {
        if rhs.degree() == 0 {
            // dividing by constant is the same as multiplying by another constant.
            // and all constant multiples of EXT_POLY are in the equivalence class
            // of 0.
            return (Element::zero(), Element::zero());
        }

        // divisor is ensured linear here.
        // now ensure divisor is monic.
        let leading_mul_inv = galois_8::div(1, rhs.0[0]);

        let monictized = rhs * leading_mul_inv;
        let mut poly = EXT_POLY;

        for i in 0..2 {
            let coef = poly[i];
            for j in 1..2 {
                if rhs.0[j] != 0 {
                    poly[i + j] ^= galois_8::mul(monictized.0[j], coef);
                }
            }
        }

        let remainder = Element::constant(poly[2]);
        let quotient = Element([poly[0], poly[1]]) * leading_mul_inv;

        (quotient, remainder)
    }

    fn polynom_div(self, rhs: Self) -> (Element, Element) {
        let divisor_degree = rhs.degree();
        if rhs.is_zero() {
            (Element::zero(), self)
        } else if self.degree() < divisor_degree {
            // If divisor's degree (len-1) is bigger, all dividend is a remainder
            (Element::zero(), self)
        } else if divisor_degree == 0 {
            // divide by constant.
            let invert = galois_8::div(1, rhs.0[1]);
            let quotient = Element([
                galois_8::mul(invert, self.0[0]),
                galois_8::mul(invert, self.0[1]),
            ]);

            (quotient, Element::zero())
        } else {
            // self degree is at least divisor degree, divisor degree not 0.
            // therefore both are 1.
            debug_assert_eq!(self.degree(), divisor_degree);
            debug_assert_eq!(self.degree(), 1);

            // ensure rhs is constant.
            let leading_mul_inv = galois_8::div(1, rhs.0[0]);
            let monic = Element([
                galois_8::mul(leading_mul_inv, rhs.0[0]),
                galois_8::mul(leading_mul_inv, rhs.0[1]),
            ]);

            let leading_coeff = self.0[0];
            let mut remainder = self.0[1];

            if monic.0[1] != 0 {
                remainder ^= galois_8::mul(monic.0[1], self.0[0]);
            }

            (
                Element::constant(galois_8::mul(leading_mul_inv, leading_coeff)),
                Element::constant(remainder),
            )
        }
    }

    /// Convert the inverse of this field element. Returns zero for zero input.
    fn inverse(self) -> Element {
        if self.is_zero() {
            return Element::zero();
        }

        // first step of extended euclidean algorithm.
        // done here because EXT_POLY is outside the scope of `Element`.
        let (gcd, y) = {
            // self / EXT_POLY = (0, self)
            let remainder = self;

            // GCD is constant because EXT_POLY is irreducible
            let (g, x, _) = remainder.const_egcd(EgcdRhs::ExtPoly);

            (g, x)
        };

        // we still need to normalize it by dividing by the gcd
        if gcd != 0 {
            // EXT_POLY is irreducible so the GCD will always be constant.
            // EXT_POLY*x + self*y = gcd
            // self*y = gcd - EXT_POLY*x
            //
            // EXT_POLY*x is representative of the equivalence class of 0.
            let normalizer = galois_8::div(1, gcd);
            y * normalizer
        } else {
            // self is equivalent to zero.
            Element::zero()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Field as _;
    use quickcheck::Arbitrary;

    impl Arbitrary for Element {
        fn arbitrary(gens: &mut quickcheck::Gen) -> Self {
            let a = u8::arbitrary(gens);
            let b = u8::arbitrary(gens);

            Element([a, b])
        }
    }

    quickcheck! {
        fn qc_add_associativity(a: Element, b: Element, c: Element) -> bool {
            a + (b + c) == (a + b) + c
        }

        fn qc_mul_associativity(a: Element, b: Element, c: Element) -> bool {
            a * (b * c) == (a * b) * c
        }

        fn qc_additive_identity(a: Element) -> bool {
            let zero = Element::zero();
            a - (zero - a) == zero
        }

        fn qc_multiplicative_identity(a: Element) -> bool {
            a.is_zero() || {
                let one = Element([0, 1]);
                (one / a) * a == one
            }
        }

        fn qc_add_commutativity(a: Element, b: Element) -> bool {
            a + b == b + a
        }

        fn qc_mul_commutativity(a: Element, b: Element) -> bool {
            a * b == b * a
        }

        fn qc_add_distributivity(a: Element, b: Element, c: Element) -> bool {
            a * (b + c) == (a * b) + (a * c)
        }

        fn qc_inverse(a: Element) -> bool {
            a.is_zero() || {
                let inv = a.inverse();
                a * inv == Element::constant(1)
            }
        }

        fn qc_exponent_1(a: Element, n: u8) -> bool {
            a.is_zero() || n == 0 || {
                let mut b = a.exp(n as usize);
                for _ in 1..n {
                    b = b / a;
                }

                a == b
            }
        }

        fn qc_exponent_2(a: Element, n: u8) -> bool {
            a.is_zero() || {
                let mut res = true;
                let mut b = Element::constant(1);

                for i in 0..n {
                    res = res && b == a.exp(i as usize);
                    b = b * a;
                }

                res
            }
        }

        fn qc_exp_zero_is_one(a: Element) -> bool {
            a.exp(0) == Element::constant(1)
        }
    }

    #[test]
    fn test_div_b_is_0() {
        assert_eq!(Element::zero(), Element([1, 0]) / Element::zero());
    }

    #[test]
    fn zero_to_zero_is_one() {
        assert_eq!(Element::zero().exp(0), Element::constant(1))
    }

    // Verifies the SIMD-decomposed `mul_slice`/`mul_slice_add` overrides against
    // the element-wise scalar `Field::mul` reference across a range of lengths
    // (including CHUNK boundary and tail) and multiplier values. If the A/B/C/D
    // tower-field decomposition were wrong, this would catch it.
    #[test]
    fn mul_slice_matches_scalar_reference() {
        const N: usize = 2100; // spans past the 1024 internal CHUNK, with a tail
        let mut input = [[0u8; 2]; N];
        for (i, e) in input.iter_mut().enumerate() {
            *e = [
                (i.wrapping_mul(31).wrapping_add(7)) as u8,
                (i.wrapping_mul(17).wrapping_add(3)) as u8,
            ];
        }

        let coeffs = [
            [0u8, 0],  // zero
            [0, 1],    // multiplicative identity
            [0, 0x8e], // pure constant coefficient
            [0x9a, 0], // pure X coefficient
            [0x9a, 0x3f],
            [0xff, 0xff],
            [0x01, 0x80],
        ];
        let lens = [0usize, 1, 2, 7, 16, 17, 63, 1023, 1024, 1025, N];

        for &c in &coeffs {
            for &len in &lens {
                let inp = &input[..len];

                // mul_slice: out = c * inp
                let mut out = [[0u8; 2]; N];
                Field::mul_slice(c, inp, &mut out[..len]);
                for (i, &e) in inp.iter().enumerate() {
                    assert_eq!(
                        out[i],
                        Field::mul(c, e),
                        "mul_slice c={c:?} len={len} i={i}"
                    );
                }

                // mul_slice_add: out ^= c * inp, starting from a nonzero seed
                let mut acc = [[0u8; 2]; N];
                for (i, e) in acc.iter_mut().enumerate() {
                    *e = [
                        (i.wrapping_mul(13).wrapping_add(5)) as u8,
                        (i.wrapping_mul(19).wrapping_add(11)) as u8,
                    ];
                }
                let seed = acc;
                Field::mul_slice_add(c, inp, &mut acc[..len]);
                for (i, &e) in inp.iter().enumerate() {
                    let expected = Field::add(seed[i], Field::mul(c, e));
                    assert_eq!(acc[i], expected, "mul_slice_add c={c:?} len={len} i={i}");
                }
            }
        }
    }
}
