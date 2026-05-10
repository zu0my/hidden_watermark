# 技术方案

## 1. 切除低频段

### 嵌入端 (`embed_watermark`)

```
移除前:
  let low_freq = get_low_freq_positions();
  let total_mid = ...;
  let total_low = blocks_x * blocks_y * low_freq.len();
  let prn_mid = generate_prng_sequence(&format!("{}_mid", key), total_mid);
  let prn_low = generate_prng_sequence(&format!("{}_low", key), total_low);
  embed_cross(&mid_freq, &prn_mid, 2.2);
  embed_cross(&low_freq, &prn_low, 0.8);

移除后:
  let prn = generate_prng_sequence(key, total_mid);
  embed_cross(&mid_freq, &prn, 2.2);
```

- `prn_mid` → `prn`，不再加 `_mid` 后缀
- 删除 `low_freq` / `prn_low` 相关行

### 检测端 (`detect_via_warp`)

无需改动——此函数从未使用低频段。

### 检测端 (`detect_via_prn_correction`)

```
移除前:
  let low_freq = get_low_freq_positions();
  let total_low = ...;
  let prn_mid = generate_prng_sequence(&format!("{}_mid", key), total_mid);
  let prn_low = generate_prng_sequence(&format!("{}_low", key), total_low);
  block_score += band_compute(&mid_freq, &prn_mid, 1.0);
  block_score += band_compute(&low_freq, &prn_low, 1.0);

移除后:
  let prn = generate_prng_sequence(key, total_mid);
  block_score = band_compute(&mid_freq, &prn, 1.0);
```

### 公共 API (`lib.rs` 顶部 re-export)

移除 `get_low_freq_positions` 的重新导出。

### `midfreq.rs` 常量

保留 `LOW_FREQ_START` / `LOW_FREQ_END` 和 `get_low_freq_positions()` 函数——它们仍有测试覆盖，且在其他代码中可能被引用（如调试输出）。只在 embedding/detection 调用端移除使用。

## 2. 消除重复：提取公共函数

### 2a. combined_prn 函数

在 `midfreq.rs` 中新增：

```rust
/// 计算十字形冗余的 PRN 组合值
/// prn: 当前 band 的 PRN 序列
/// (by, bx): 当前块坐标
/// (blocks_x, blocks_y): 块网格尺寸
/// band_len: 每个 block 的 band 系数个数
/// i: band 内索引
/// base: 预计算的 (by * blocks_x * band_len + bx * band_len + i)，传 0 则 auto
pub fn combined_prn(
    prn: &[f64],
    blocks_x: usize, blocks_y: usize,
    band_len: usize,
    by: usize, bx: usize,
    i: usize,
) -> f64 {
    let base = by * blocks_x * band_len + bx * band_len + i;
    let own = prn[base];
    let left = if bx > 0 { prn[by * blocks_x * band_len + (bx - 1) * band_len + i] } else { 0.0 };
    let top = if by > 0 { prn[(by - 1) * blocks_x * band_len + bx * band_len + i] } else { 0.0 };
    let right = if bx + 1 < blocks_x { prn[by * blocks_x * band_len + (bx + 1) * band_len + i] } else { 0.0 };
    let bottom = if by + 1 < blocks_y { prn[(by + 1) * blocks_x * band_len + bx * band_len + i] } else { 0.0 };
    own + left + top + right + bottom
}
```

嵌入端和检测端的 3 处重复替换为调用此函数。

### 2b. map_corners_to_weights 函数

在 `lib.rs` 中新增私有函数（`detect_via_prn_correction` 上方）：

```rust
/// 将 suspect 块的 4 个角点通过变换映射到原始空间，
/// 返回与原始 DCT 块重叠的面积加权列表
fn map_corners_to_weights(
    bx: usize, by: usize,
    transform: &[f32; 9],
    blocks_x: usize, blocks_y: usize,
) -> Vec<(usize, f64)> {
    // ... 角点映射 + 面积加权逻辑 ...
    // 返回 Vec<(block_index, weight)>，权重已归一化
}
```

替换 `detect_via_prn_correction` 中两处重复的角点映射代码。

## 3. 清理死代码

### 移除 `transform_block_coord`

- 从 `align.rs` 中删除此函数
- 从 `lib.rs` 的 `pub use` 中移除

### 移除 `lazy_static` 依赖

检查是否仍被使用——若仅用于已删除的代码，从 `Cargo.toml` 移除。

## 影响面

| 文件 | 改动类型 |
|------|---------|
| `src/lib.rs` | 删低频段调用、删 dead code re-export、提取公共函数、替换重复代码 |
| `src/midfreq.rs` | 新增 `combined_prn` 函数 |
| `src/align.rs` | 删除 `transform_block_coord` |
| `Cargo.toml` | 可能移除 `lazy_static` |
