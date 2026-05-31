# Sage — Galois Field Mathematical Verification

[English](README.md) | [中文](README_CN.md)

This directory contains [SageMath](https://www.sagemath.org/) scripts for **independent symbolic verification** of the Galois finite field arithmetic used in the Reed-Solomon erasure coding library.

SageMath is a Python-based open-source mathematical computation system that provides exact finite field algebraic operations without floating-point approximation.

---

## Directory Structure

```
sage/
├── README.md              # English documentation
├── README_CN.md           # Chinese documentation (中文文档)
└── galois_ext_test.sage   # GF(2^16) extension field arithmetic verification
```

---

## Background: Finite Fields in Reed-Solomon Coding

The core operations of Reed-Solomon coding take place over **finite fields (Galois Fields)**. This project supports two finite fields:

| Field | Symbol | Elements | Representation | Max Shards | Rust Module |
|---|---|---|---|---|---|
| GF(2^8) | `GF256` | 256 | `u8` | 256 | `src/galois_8/` |
| GF(2^16) | `GF65536` | 65536 | `[u8; 2]` | 65536 | `src/galois_16.rs` |

### GF(2^8) — Base Field

GF(2^8) is a finite field with 256 elements, constructed as:

```
GF(2^8) = GF(2)[a] / (a^8 + a^4 + a^3 + a + 1)
```

- **Irreducible polynomial**: `a^8 + a^4 + a^3 + a + 1` (hex `0x11B`, generating polynomial constant `29 = 0x1D` in this project)
- **Primitive element**: `a`, satisfying `a^8 = a^4 + a^3 + a + 1`
- **Element representation**: Each element is a polynomial in `a` of degree 0–7 with coefficients in GF(2), i.e., an 8-bit binary number
- **Operations**: Addition is XOR; multiplication is polynomial multiplication modulo the irreducible polynomial

The project's `build.rs` generates GF(2^8) log tables and exp tables at compile time for efficient multiplication/division (converting multiplication to addition via table lookup).

### GF(2^16) — Extension Field

GF(2^16) = GF((2^8)^2) is a degree-2 extension of GF(2^8), constructed as:

```
GF(2^16) = GF(2^8)[b] / (b^2 + a*b + a^7)
```

- **Base field**: GF(2^8)
- **Irreducible polynomial**: `b^2 + a*b + a^7` (irreducible over GF(2^8)[b])
- **Element representation**: `c1*b + c0`, where `c1, c0 ∈ GF(2^8)`
- **Number of elements**: 256^2 = 65536
- **Max shards**: 65536 (far exceeding GF(2^8)'s 256)

---

## `galois_ext_test.sage` — Detailed Analysis

### Purpose

Verifies the correctness of basic arithmetic operations (addition, multiplication, division, inversion) in the GF(2^16) extension field. The script's output serves as **golden vectors** for the Rust implementation (`src/galois_16.rs`).

### Line-by-Line Walkthrough

#### Part 1: Constructing the GF(2^8) Base Field

```sage
GF256.<a> = FiniteField(256)
```

- Creates GF(2^8) = GF(256) with primitive element named `a`
- SageMath uses the Conway polynomial by default (for GF(256): `a^8 + a^4 + a^3 + a + 1`)
- `a` satisfies `a^8 = a^4 + a^3 + a + 1`, i.e., `a^255 = 1`

#### Part 2: Constructing the Polynomial Ring and Extension Field

```sage
R.<x> = GF256[x]
ext_poly = R.irreducible_element(2, algorithm="first_lexicographic")
ExtField.<b> = GF256.extension(ext_poly)
```

- `R.<x> = GF256[x]`: Constructs a polynomial ring over GF(256)
- `irreducible_element(2, algorithm="first_lexicographic")`: Selects the first degree-2 irreducible polynomial in lexicographic order
- Result: `x^2 + a*x + a^7`
- `GF256.extension(ext_poly)`: Constructs the degree-2 extension field of GF(256)
- `b` is the extension field generator, satisfying `b^2 + a*b + a^7 = 0`

#### Part 3: Extension Field Element Definitions

```sage
e1 = (a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1
e2 = (a^7 + a^5 + a^2)*b + a^7 + a^4 + a^3 + a
```

General elements in the extension field have the form `c1*b + c0`:
- `e1`: leading coefficient `c1 = a^7 + a^6 + a^4 + a`, constant term `c0 = a^3 + a^2 + a + 1`
- `e2`: leading coefficient `c1 = a^7 + a^5 + a^2`, constant term `c0 = a^7 + a^4 + a^3 + a`

Each coefficient is itself an element of GF(2^8), i.e., a polynomial in `a`.

#### Part 4: Arithmetic Operation Verification

```sage
print "e1 + e2: ", e1 + e2
#(a^6 + a^5 + a^4 + a^2 + a)*b + a^7 + a^4 + a^2 + 1
```

**Addition**: Component-wise XOR (addition in characteristic 2 is XOR).
```
(c1*b + c0) + (d1*b + d0) = (c1⊕d1)*b + (c0⊕d0)
```

```sage
print "e1 * e2: ", e1 * e2
#(a^4 + a^2 + a + 1)*b + a^7 + a^5 + a^3 + a
```

**Multiplication**: Polynomial multiplication followed by reduction modulo `b^2 + a*b + a^7`.
```
(c1*b + c0) * (d1*b + d0)
= c1*d1*b^2 + (c1*d0 + c0*d1)*b + c0*d0
```
Since `b^2 = a*b + a^7` (because `b^2 + a*b + a^7 = 0`):
```
= c1*d1*(a*b + a^7) + (c1*d0 + c0*d1)*b + c0*d0
= (c1*d1*a + c1*d0 + c0*d1)*b + (c1*d1*a^7 + c0*d0)
```
All coefficient arithmetic is performed in GF(2^8).

```sage
print "e1 / e2: ", e1 / e2
#(a^7 + a^6 + a^5 + a^4 + a^3 + a^2 + 1)*b + a^6 + a^3 + a
```

**Division**: `e1 * e2^{-1}` — first computes the multiplicative inverse of `e2`, then multiplies. The inverse is computed via the extended Euclidean algorithm.

```sage
print "1/b: ", 1/b
#(a^4 + a^3 + a + 1)*b + a^5 + a^4 + a^2 + a
```

**Inversion**: Verifies that `b` has a computable multiplicative inverse. `b * (1/b) = 1`.

---

## Mapping to the Rust Implementation

### Irreducible Polynomial

The irreducible polynomial `x^2 + a*x + a^7` selected in the SageMath script is hardcoded in the Rust source:

```rust
// src/galois_16.rs, line 14
const EXT_POLY: [u8; 3] = [1, 2, 128];
```

Mapping:
| Coefficient | SageMath | u8 Value | Notes |
|---|---|---|---|
| `x^2` coeff | `1` | `1` | Leading coefficient (monic polynomial) |
| `x^1` coeff | `a` | `2` | `a` in GF(2^8) is `0x02` |
| `x^0` coeff | `a^7` | `128` | `a^7 = 0x80 = 128` |

### Element Representation

| SageMath | Rust | Notes |
|---|---|---|
| `(a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1` | `Element([0b10110010, 0b00001111])` | Leading coeff in `[0]`, constant in `[1]` |
| `c1*b + c0` | `Element([c1, c0])` | `[u8; 2]` array |

### Operation Mapping

| Operation | SageMath | Rust (`src/galois_16.rs`) |
|---|---|---|
| Addition | `e1 + e2` | `Element::add()` — component XOR (lines 126–131) |
| Multiplication | `e1 * e2` | `Element::mul()` — FOIL expansion + `reduce_from()` (lines 142–158) |
| Division | `e1 / e2` | `Element::div()` — `self * rhs.inverse()` (lines 168–175) |
| Inversion | `1/e` | `Element::inverse()` — extended Euclidean algorithm (lines 282–312) |

### Multiplication Core Algorithm

The Rust multiplication implementation (`Element::mul`, lines 145–157):

```rust
fn mul(self, rhs: Self) -> Element {
    // FOIL: (c1*b + c0) * (d1*b + d0)
    let out: [u8; 3] = [
        galois_8::mul(self.0[0], rhs.0[0]),           // c1*d1 → b^2 term
        galois_8::add(
            galois_8::mul(self.0[1], rhs.0[0]),        // c0*d1 → b term
            galois_8::mul(self.0[0], rhs.0[1]),        // c1*d0 → b term
        ),
        galois_8::mul(self.0[1], rhs.0[1]),            // c0*d0 → constant term
    ];
    Element::reduce_from(out)  // reduce modulo b^2 + a*b + a^7
}
```

`reduce_from` (lines 97–107) performs polynomial reduction:
```rust
fn reduce_from(mut x: [u8; 3]) -> Self {
    if x[0] != 0 {
        // b^2 ≡ a*b + a^7 (mod ext_poly)
        x[1] ^= galois_8::mul(EXT_POLY[1], x[0]);  // += a * x[0]
        x[2] ^= galois_8::mul(EXT_POLY[2], x[0]);  // += a^7 * x[0]
    }
    Element([x[1], x[2]])
}
```

This is equivalent to the `e1 * e2` computation in SageMath.

### Inversion Core Algorithm

The Rust inverse computation (`Element::inverse`, lines 282–312) uses the **extended Euclidean algorithm**:

```
EXT_POLY * x + self * y = gcd(EXT_POLY, self)
```

Since `EXT_POLY` is irreducible, `gcd` is always a constant, so:
```
self * y ≡ gcd (mod EXT_POLY)
self^{-1} = y / gcd
```

SageMath's `1/e` operation uses the same algorithm internally.

---

## Running the Scripts

### Installing SageMath

```bash
# macOS (Homebrew)
brew install sagemath

# Or via conda
conda install -c conda-forge sage

# Ubuntu/Debian
sudo apt install sagemath
```

### Running the Script

```bash
cd sage
sage galois_ext_test.sage
```

### Expected Output

```
Finite Field in b of size 2^16
65536
e1:  (a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1
e2:  (a^7 + a^5 + a^2)*b + a^7 + a^4 + a^3 + a
e1 + e2:  (a^6 + a^5 + a^4 + a^2 + a)*b + a^7 + a^4 + a^2 + 1
e1 * e2:  (a^4 + a^2 + a + 1)*b + a^7 + a^5 + a^3 + a
e1 / e2:  (a^7 + a^6 + a^5 + a^4 + a^3 + a^2 + 1)*b + a^6 + a^3 + a
1/b:  (a^4 + a^3 + a + 1)*b + a^5 + a^4 + a^2 + a
```

---

## Golden Vectors

The expected output in the script comments can serve as golden vectors for Rust unit tests. For example, converting `e1 * e2` to Rust:

```rust
// e1 = (a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1
// a^7 + a^6 + a^4 + a = 0b10110010 = 0xB2
// a^3 + a^2 + a + 1   = 0b00001111 = 0x0F
let e1 = Element([0xB2, 0x0F]);

// e2 = (a^7 + a^5 + a^2)*b + a^7 + a^4 + a^3 + a
// a^7 + a^5 + a^2     = 0b10100100 = 0xA4
// a^7 + a^4 + a^3 + a = 0b10011010 = 0x9A
let e2 = Element([0xA4, 0x9A]);

// e1 * e2 = (a^4 + a^2 + a + 1)*b + a^7 + a^5 + a^3 + a
// a^4 + a^2 + a + 1   = 0b00010111 = 0x17
// a^7 + a^5 + a^3 + a = 0b10101010 = 0xAA
let expected = Element([0x17, 0xAA]);

assert_eq!(e1 * e2, expected);
```

---

## Extension: Additional Verification Scenarios

Beyond the existing tests in `galois_ext_test.sage`, SageMath can be used for:

| Scenario | Description |
|---|---|
| **Encoding matrix verification** | Verify Vandermonde / Cauchy encoding matrices over GF(2^8) |
| **Irreducible polynomial selection** | Explore different irreducible polynomials over GF(2^8) and their performance characteristics |
| **Field isomorphism verification** | Verify equivalence between different representations (polynomial / matrix / lookup table) |
| **Leopard codec verification** | Verify Leopard-GF8 codec butterfly networks and FFT structures |
| **Error injection testing** | Construct specific error patterns to verify reconstruction algorithm boundary behavior |

---

## References

- [SageMath Finite Fields Documentation](https://doc.sagemath.org/html/en/reference/finite_rings/sage/rings/finite_rings/finite_field_constructor.html)
- [Reed-Solomon Error Correction — Wikipedia](https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction)
- [Finite Field Arithmetic — Wikipedia](https://en.wikipedia.org/wiki/Finite_field_arithmetic)
- `src/galois_16.rs` — Rust implementation of GF(2^16)
- `src/galois_8/` — Rust implementation of GF(2^8) (with SIMD acceleration)
- `build.rs` — Compile-time generation of GF(2^8) log/exp tables
