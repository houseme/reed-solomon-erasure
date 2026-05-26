# 阶段 2：API 与配置能力补齐

## 1. 阶段目标

把当前 crate 从“核心算法库”提升到“具备现代工程可用性的编码器库”。

重点补齐：

- options/config 层
- reconstruction 细粒度接口
- 便捷切片接口
- 单 parity 快路径
- cache 行为可配置

## 2. 交付物

1. `CodecOptions` 或 Builder 模式
2. `reconstruct_some`
3. `split` / `join`
4. `fast_one_parity`
5. inversion cache 配置开关

## 2.1 推荐实现顺序

建议阶段 2 内按如下顺序落地：

1. `CodecOptions`
2. `with_options`
3. inversion cache 开关
4. `fast_one_parity`
5. `split` / `join`
6. `reconstruct_some`

原因：

- 先有 options，后面的行为开关才有干净入口
- `fast_one_parity` 依赖 options 暴露
- `reconstruct_some` 复杂度最高，适合放在本阶段后半段

## 3. API 设计建议

### 设计目标

- 不破坏现有 `ReedSolomon::new(data, parity)` 用法
- 提供扩展构造方式
- 保持 `no_std` 兼容

### 推荐接口

```rust
pub struct CodecOptions {
    pub fast_one_parity: bool,
    pub inversion_cache: bool,
    pub inversion_cache_capacity: usize,
    pub matrix_mode: MatrixMode,
}
```

```rust
pub enum MatrixMode {
    Vandermonde,
    Cauchy,
    JerasureLike,
    Custom,
}
```

```rust
impl<F: Field> ReedSolomon<F> {
    pub fn with_options(
        data_shards: usize,
        parity_shards: usize,
        options: CodecOptions,
    ) -> Result<Self, Error>;
}
```

### 推荐默认语义

建议 `Default` 语义如下：

```rust
impl Default for CodecOptions {
    fn default() -> Self {
        Self {
            fast_one_parity: false,
            inversion_cache: true,
            inversion_cache_capacity: 254,
            matrix_mode: MatrixMode::Vandermonde,
        }
    }
}
```

设计原则：

1. 默认行为尽量与当前实现一致
2. 不因为引入 options 而改变用户现有结果
3. 未来新增配置项也应遵循“保守默认值”

### `ReedSolomon::new` 与 `with_options` 的关系

推荐：

```rust
pub fn new(data_shards: usize, parity_shards: usize) -> Result<Self, Error> {
    Self::with_options(data_shards, parity_shards, CodecOptions::default())
}
```

这样可避免双份初始化逻辑分叉。

## 4. 任务拆解

### 任务 1：引入 options 层

要求：

- 默认行为与当前版本一致
- 配置项有合理默认值
- 所有新增选项都可单元测试

建议补充字段：

```rust
pub struct CodecOptions {
    pub fast_one_parity: bool,
    pub inversion_cache: bool,
    pub inversion_cache_capacity: usize,
    pub matrix_mode: MatrixMode,
}
```

本阶段不建议提前加入：

- auto_threads
- runtime backend 选择
- SIMD 强制开关

这些属于后续阶段职责，提前塞进 options 会让边界变乱。

### 任务 2：实现 `reconstruct_some`

目标：

- 只恢复指定缺失 shard
- 避免不需要的 parity/data 全量重建

建议签名：

```rust
pub fn reconstruct_some<T: ReconstructShard<F>>(
    &self,
    shards: &mut [T],
    required: &[bool],
) -> Result<(), Error>;
```

要求：

- `required.len()` 必须与 `total_shards` 一致
- 若只要求少量 shard，应尽量少做输出写回

### `reconstruct_some` 的推荐语义

`required[i] == true` 表示：

- 若该 shard 已存在，则无需修改
- 若该 shard 缺失，则本次必须恢复

`required[i] == false` 表示：

- 即便该 shard 缺失，也不要求本次恢复

### `reconstruct_some` 的推荐内部流程

建议流程：

1. 做与 `reconstruct_internal` 相同的输入合法性校验
2. 收集 present/missing shard 信息
3. 计算 data decode matrix
4. 只恢复：
   - 被标记为 `required=true` 的 data shard
   - 若某些 parity shard 被显式要求，也只恢复这些 parity shard
5. 不要求恢复的缺失 shard 不初始化、不写回

### 关键实现注意点

必须处理好以下情况：

1. `required` 中要求恢复的 data shard 本身影响后续其他 required shard 的恢复
2. 只要求恢复 parity shard 时，仍可能需要先临时恢复部分 data 路径依赖
3. 不允许为不必要的缺失 parity shard 做额外分配

### 推荐实现策略

第一版建议限制为：

- 只对 required data shard 做最小恢复
- parity shard 的 partial recovery 先支持，但实现可以保守

如果实现复杂度过高，可在第一版中明确：

- `reconstruct_some` 优先优化 data shard 场景

### 推荐测试矩阵

至少覆盖：

1. 仅缺 1 个 data shard，且 required 指向它
2. 缺多个 data shard，但只要求恢复其中 1 个
3. 缺 data + parity，且只要求恢复 data
4. 缺多个 parity，且只要求恢复 1 个 parity
5. `required.len()` 错误
6. required 标记了已存在 shard
7. requested shards 超出可恢复能力

### 任务 3：实现 `split` / `join`

作用：

- 降低上层接入成本
- 与 MinIO 路线中的 `Split` 使用习惯对齐

建议：

- `split(data: &[u8]) -> Result<Vec<Vec<F::Elem>>, Error>`
- `join(shards: &[impl AsRef<[F::Elem]>], out_len: usize) -> Result<Vec<F::Elem>, Error>`

### `split` 的建议语义

目标：

- 与 data shard count 对齐切分
- 自动补零到 shard 边界

输出建议：

- 返回完整 data shards
- parity shards 由调用者后续通过 `encode` 生成

### `join` 的建议语义

目标：

- 将 data shards 顺序拼回
- 按 `out_len` 截断尾部 padding

边界条件：

- `out_len == 0`
- shard 长度不一致
- shard 数量不足

### 任务 4：`fast_one_parity`

当 `parity_shards == 1` 时：

- 可直接走 XOR 路线
- 不需要一般化矩阵乘法路径

收益：

- 实现简单
- ROI 高
- 对标 `WithFastOneParityMatrix`

### 推荐实现策略

当 `parity_shards == 1 && fast_one_parity == true` 时：

- encode 走 XOR 快路径
- verify 走 XOR 对照
- reconstruct 仍然优先复用通用路径，避免第一版改动过宽

建议实现顺序：

1. encode 快路径
2. verify 快路径
3. 后续再评估 reconstruct 是否值得专门特化

### 推荐测试矩阵

至少覆盖：

1. `parity=1` 快路径与通用路径输出一致
2. verify 对正确 parity 返回 true
3. verify 对损坏 parity 返回 false
4. 不启用 `fast_one_parity` 时行为与旧版本一致

### 任务 5：cache 开关与容量控制

要求：

- 可启用/禁用重建矩阵缓存
- 可配置容量
- 默认行为与当前兼容

### 推荐行为

当：

- `inversion_cache == false`

则：

- reconstruction 时不读 cache
- reconstruction 后不写 cache

当：

- `inversion_cache == true`

则：

- 读写 cache 均启用
- `inversion_cache_capacity == 0` 应返回错误或被规范化为默认值

建议优先：

- 在 `with_options` 构造阶段就完成容量合法化

## 5. 验收标准

1. 现有 API 行为不变
2. 新 API 有完整单元测试
3. `fast_one_parity` 路径结果与通用路径一致
4. `reconstruct_some` 在随机测试中行为正确
5. `CodecOptions::default()` 明确且稳定
6. `split` / `join` 对 padding 行为有覆盖测试

## 6. 风险点

- 新 API 过多会让构造方式复杂化
- 自定义矩阵若过早引入，可能拖慢本阶段落地

## 7. 风险应对

- 本阶段先只把 `MatrixMode` 设计好
- Cauchy/JerasureLike 可先定义接口，不强求一步到位全部实现

额外建议：

- `Custom` matrix 可以先不开放公开构造
- 若开放，也应先限制维度校验和一致性校验

## 8. 完成后的收益

- 为并行、SIMD、缓存优化提供干净的配置入口
- 为上层系统接入提供更现代的 API 体验

同时还能为后续阶段提供：

- 稳定的 feature 接入口
- 更细粒度的 reconstruction 优化空间
- 更明确的行为契约
