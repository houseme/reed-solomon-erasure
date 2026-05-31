# Sage — Galois 域数学验证工具

[English](README.md) | 中文

本目录包含 [SageMath](https://www.sagemath.org/) 脚本，用于对 Reed-Solomon 纠删码中使用的 Galois 有限域算术进行**独立的符号验证**。

SageMath 是一个基于 Python 的开源数学计算系统，提供精确的有限域代数运算能力，不依赖浮点近似。

---

## 目录结构

```
sage/
├── README.md              # 英文文档
├── README_CN.md           # 中文文档（本文件）
└── galois_ext_test.sage   # GF(2^16) 扩展域算术验证脚本
```

---

## 背景：Reed-Solomon 编码中的有限域

Reed-Solomon 编码的核心运算发生在**有限域 (Galois Field)** 上。本项目支持两种有限域：

| 域 | 符号 | 元素个数 | 元素表示 | 最大分片数 | Rust 模块 |
|---|---|---|---|---|---|
| GF(2^8) | `GF256` | 256 | `u8` | 256 | `src/galois_8/` |
| GF(2^16) | `GF65536` | 65536 | `[u8; 2]` | 65536 | `src/galois_16.rs` |

### GF(2^8) — 基域

GF(2^8) 是一个包含 256 个元素的有限域，构造方式为：

```
GF(2^8) = GF(2)[a] / (a^8 + a^4 + a^3 + a + 1)
```

- **不可约多项式**：`a^8 + a^4 + a^3 + a + 1`（十六进制表示为 `0x11B`，本项目中生成多项式常数为 `29 = 0x1D`）
- **本原元**：`a`，满足 `a^8 = a^4 + a^3 + a + 1`
- **元素表示**：每个元素是 `a` 的 0~7 次多项式，系数在 GF(2) 中，即一个 8 位二进制数
- **运算规则**：加法为 XOR，乘法为多项式乘法后模不可约多项式

本项目的 `build.rs` 在编译时生成 GF(2^8) 的对数表 (log table) 和指数表 (exp table)，用于高效的乘法/除法运算（通过查表将乘法转换为加法）。

### GF(2^16) — 扩展域

GF(2^16) = GF((2^8)^2) 是 GF(2^8) 的 2 次扩展，构造方式为：

```
GF(2^16) = GF(2^8)[b] / (b^2 + a*b + a^7)
```

- **基域**：GF(2^8)
- **不可约多项式**：`b^2 + a*b + a^7`（在 GF(2^8)[b] 上不可约）
- **元素表示**：`c1*b + c0`，其中 `c1, c0 ∈ GF(2^8)`
- **元素数量**：256^2 = 65536
- **最大分片数**：65536（远超 GF(2^8) 的 256）

---

## `galois_ext_test.sage` 详解

### 作用

验证 GF(2^16) 扩展域的基本算术运算（加法、乘法、除法、求逆）的正确性。该脚本的输出可作为 Rust 实现 (`src/galois_16.rs`) 的**黄金向量 (golden vectors)**。

### 逐行解读

#### 第一部分：构建 GF(2^8) 基域

```sage
GF256.<a> = FiniteField(256)
```

- 创建 GF(2^8) = GF(256)，本原元命名为 `a`
- SageMath 默认使用 Conway 多项式（对于 GF(256) 是 `a^8 + a^4 + a^3 + a + 1`）
- `a` 满足 `a^8 = a^4 + a^3 + a + 1`，即 `a^255 = 1`

#### 第二部分：构建多项式环和扩展域

```sage
R.<x> = GF256[x]
ext_poly = R.irreducible_element(2, algorithm="first_lexicographic")
ExtField.<b> = GF256.extension(ext_poly)
```

- `R.<x> = GF256[x]`：在 GF(256) 上构造多项式环
- `irreducible_element(2, algorithm="first_lexicographic")`：选取次数为 2 的第一个不可约多项式（字典序）
- 结果为 `x^2 + a*x + a^7`
- `GF256.extension(ext_poly)`：构造 GF(256) 的 2 次扩展域
- `b` 是扩展域的生成元，满足 `b^2 + a*b + a^7 = 0`

#### 第三部分：扩展域元素定义

```sage
e1 = (a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1
e2 = (a^7 + a^5 + a^2)*b + a^7 + a^4 + a^3 + a
```

扩展域中的一般元素形如 `c1*b + c0`：
- `e1`：高次系数 `c1 = a^7 + a^6 + a^4 + a`，常数项 `c0 = a^3 + a^2 + a + 1`
- `e2`：高次系数 `c1 = a^7 + a^5 + a^2`，常数项 `c0 = a^7 + a^4 + a^3 + a`

每个系数本身是 GF(2^8) 中的元素，即 `a` 的多项式。

#### 第四部分：算术运算验证

```sage
print "e1 + e2: ", e1 + e2
#(a^6 + a^5 + a^4 + a^2 + a)*b + a^7 + a^4 + a^2 + 1
```

**加法**：分量逐位 XOR（因为 GF(2) 特征下加法即 XOR）。
```
(c1*b + c0) + (d1*b + d0) = (c1⊕d1)*b + (c0⊕d0)
```

```sage
print "e1 * e2: ", e1 * e2
#(a^4 + a^2 + a + 1)*b + a^7 + a^5 + a^3 + a
```

**乘法**：多项式乘法后模 `b^2 + a*b + a^7` 约化。
```
(c1*b + c0) * (d1*b + d0)
= c1*d1*b^2 + (c1*d0 + c0*d1)*b + c0*d0
```
其中 `b^2 = a*b + a^7`（因为 `b^2 + a*b + a^7 = 0`），所以：
```
= c1*d1*(a*b + a^7) + (c1*d0 + c0*d1)*b + c0*d0
= (c1*d1*a + c1*d0 + c0*d1)*b + (c1*d1*a^7 + c0*d0)
```
所有系数运算在 GF(2^8) 中进行。

```sage
print "e1 / e2: ", e1 / e2
#(a^7 + a^6 + a^5 + a^4 + a^3 + a^2 + 1)*b + a^6 + a^3 + a
```

**除法**：`e1 * e2^{-1}`，先求 `e2` 的乘法逆元，再做乘法。逆元通过扩展欧几里得算法计算。

```sage
print "1/b: ", 1/b
#(a^4 + a^3 + a + 1)*b + a^5 + a^4 + a^2 + a
```

**求逆**：验证 `b` 的乘法逆元存在且可计算。`b * (1/b) = 1`。

---

## 与 Rust 实现的对应关系

### 不可约多项式

SageMath 脚本中选取的不可约多项式 `x^2 + a*x + a^7` 直接硬编码在 Rust 代码中：

```rust
// src/galois_16.rs, 第 14 行
const EXT_POLY: [u8; 3] = [1, 2, 128];
```

对应关系：
| 系数 | SageMath 表示 | u8 值 | 说明 |
|---|---|---|---|
| `x^2` 系数 | `1` | `1` | 最高次项系数（始终为 1，首一多项式） |
| `x^1` 系数 | `a` | `2` | `a` 在 GF(2^8) 中的表示为 `0x02` |
| `x^0` 系数 | `a^7` | `128` | `a^7 = 0x80 = 128` |

### 元素表示

| SageMath | Rust | 说明 |
|---|---|---|
| `(a^7 + a^6 + a^4 + a)*b + a^3 + a^2 + a + 1` | `Element([0b10110010, 0b00001111])` | 高次系数在 `[0]`，常数项在 `[1]` |
| `c1*b + c0` | `Element([c1, c0])` | `[u8; 2]` 数组 |

### 运算实现

| 运算 | SageMath | Rust (`src/galois_16.rs`) |
|---|---|---|
| 加法 | `e1 + e2` | `Element::add()` — 分量 XOR（第 126-131 行） |
| 乘法 | `e1 * e2` | `Element::mul()` — FOIL 展开 + `reduce_from()`（第 142-158 行） |
| 除法 | `e1 / e2` | `Element::div()` — `self * rhs.inverse()`（第 168-175 行） |
| 求逆 | `1/e` | `Element::inverse()` — 扩展欧几里得算法（第 282-312 行） |

### 乘法的核心算法

Rust 中的乘法实现（`Element::mul`，第 145-157 行）：

```rust
fn mul(self, rhs: Self) -> Element {
    // FOIL 展开：(c1*b + c0) * (d1*b + d0)
    let out: [u8; 3] = [
        galois_8::mul(self.0[0], rhs.0[0]),           // c1*d1 → b^2 项
        galois_8::add(
            galois_8::mul(self.0[1], rhs.0[0]),        // c0*d1 → b 项
            galois_8::mul(self.0[0], rhs.0[1]),        // c1*d0 → b 项
        ),
        galois_8::mul(self.0[1], rhs.0[1]),            // c0*d0 → 常数项
    ];
    Element::reduce_from(out)  // 模 b^2 + a*b + a^7 约化
}
```

`reduce_from`（第 97-107 行）执行多项式约化：
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

这等价于 SageMath 中 `e1 * e2` 的计算过程。

### 求逆的核心算法

Rust 中的逆元计算（`Element::inverse`，第 282-312 行）使用**扩展欧几里得算法**：

```
EXT_POLY * x + self * y = gcd(EXT_POLY, self)
```

由于 `EXT_POLY` 不可约，`gcd` 必为常数，因此：
```
self * y ≡ gcd (mod EXT_POLY)
self^{-1} = y / gcd
```

SageMath 中的 `1/e` 运算在底层使用相同的算法。

---

## 如何运行

### 安装 SageMath

```bash
# macOS (Homebrew)
brew install sagemath

# 或通过 conda
conda install -c conda-forge sage

# Ubuntu/Debian
sudo apt install sagemath
```

### 运行脚本

```bash
cd sage
sage galois_ext_test.sage
```

### 预期输出

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

## 作为黄金向量

脚本注释中的预期输出可用作 Rust 单元测试的黄金向量。例如，将 `e1 * e2` 的结果转换为 Rust：

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

## 扩展：可用于验证的其他场景

除了 `galois_ext_test.sage` 中已有的测试，SageMath 还可用于：

| 场景 | 说明 |
|---|---|
| **编码矩阵验证** | 验证 Vandermonde / Cauchy 编码矩阵在 GF(2^8) 上的正确性 |
| **不可约多项式选取** | 探索 GF(2^8) 上不同的不可约多项式及其性能特征 |
| **域同构验证** | 验证不同表示（多项式 / 矩阵 / 查表）之间的等价性 |
| **Leopard 编码验证** | 验证 Leopard-GF8 编解码器的 butterfly 网络和 FFT 结构 |
| **错误注入测试** | 构造特定的错误模式，验证重建算法的边界行为 |

---

## 参考资料

- [SageMath 有限域文档](https://doc.sagemath.org/html/en/reference/finite_rings/sage/rings/finite_rings/finite_field_constructor.html)
- [Reed-Solomon 纠删码 — Wikipedia](https://en.wikipedia.org/wiki/Reed%E2%80%93Solomon_error_correction)
- [Galois 域算术 — Wikipedia](https://en.wikipedia.org/wiki/Finite_field_arithmetic)
- 本项目 `src/galois_16.rs` — GF(2^16) 的 Rust 实现
- 本项目 `src/galois_8/` — GF(2^8) 的 Rust 实现（含 SIMD 加速）
- 本项目 `build.rs` — GF(2^8) 对数/指数表的编译时生成
