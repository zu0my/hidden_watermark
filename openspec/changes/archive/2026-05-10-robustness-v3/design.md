# Robustness v3: 技术方案

## Step 1: 去除低频段

### 嵌入端

删除 `embed_watermark` 中的低频段嵌入：

```diff
- embed_band(&low_freq, &prn_low, coeff_start_low, 0.5, ...);
- let low_freq = get_low_freq_positions();
- let total_low = ...;
- let prn_low = generate_prng_sequence(&format!("{}_low", key), total_low);
```

只保留中频段嵌入，强度调至 1.5：

```rust
embed_band(&mid_freq, &prn, coeff_start, 1.5, ...);
```

### 检测端

删除传统路径和 PRN 校正路径中的低频段 score 计算，只保留中频。同时删除 `get_low_freq_positions` 和 `get_high_freq_positions`（未使用的代码）。

## Step 2: 混合对齐策略

### 路由逻辑

```
detect_watermark(original, suspect, key, fpr):
  alignment = align_images(original, suspect)
  
  if alignment.confidence < 0.2:
    return not_detected
  
  // 判断对齐类型
  angle = |alignment.rotation|
  scale_diff = |alignment.scale - 1.0|
  
  if angle <= 3.0 && scale_diff < 0.05:
    // 小角度纯旋转 → warp 路径
    return detect_via_warp(original, suspect, key, fpr, alignment)
  else:
    // 大角度或缩放 → PRN 校正路径
    return detect_via_prn_correction(original, suspect, key, fpr, alignment)
```

### 为什么 3°

```
  1°: warp 偏移小，DCT 块内亚像素误差 < 0.28px → 信号损失可接受
  3°: warp 偏移中等，边缘失真约 3% 区域 → 中心 50% 区域避开
  5°: warp 偏移大，边缘失真 > 5% → 用 PRN 校正
```

`detect_via_warp` 就是当前的 warp 路径（`align_images` 返回的 `aligned` 图像 + 传统 DCT 块比对 + 块冗余）。

## Step 3: 增强信号强度 + 块冗余

### 嵌入强度

```rust
// 之前: alpha = strength * weight * 1.0 * 0.5
// 之后: alpha = strength * weight * 1.5 * 0.5
```

中频嵌入系数从 1.0 提升至 1.5。

PSNR 预计仍 > 40 dB（之前 0.8× 强度下 PSNR 远超 40）。

### 十字形块冗余（5 块）

```
当前（3 块):
  主块 (bx, by)
  左邻居 (bx-1, by)
  上邻居 (bx, by-1)

升级后（5 块):
  主块 (bx, by)
  左邻居 (bx-1, by)
  上邻居 (bx, by-1)
  右邻居 (bx+1, by)   ← 新增
  下邻居 (bx, by+1)   ← 新增
```

嵌入时：`combined = own_prn + left_prn + top_prn + right_prn + bottom_prn`
检测时：同样的加权（全 1.0，保持一致）

### 对抗裁剪的原理

```
以前 (3 块):
  裁剪 75% 保留率 → 主块可能被切掉，但左或上邻居之一还在 → 有信号
  但上下只覆盖两个方向

现在 (5 块, 十字形):
  裁剪 75% → 5 个方向覆盖 → 最少有 2 个块在保留区域内
  真正的 4 方向冗余 → 抗裁剪显著增强
```

## 数据流变化

```
变化前 (v2):
  suspect → ORB 对齐 → PRN 校正 / warp → 低频+中频检测 → 3 块冗余

变化后 (v3):
  suspect → ORB 对齐
              ├─ |角度| ≤ 3° → warp → 中频检测 → 5 块冗余
              └─ |角度| > 3° → PRN 校正 → 中频检测 → 5 块冗余
```

## 预估效果

| 攻击 | v2 ratio | 预期 v3 ratio | 关键变化 |
|------|----------|---------------|---------|
| 干净 | 2.09x | > 4.0x | 去低频 + 增强 1.5x |
| 旋转 1-3° | -0.04x | > 1.5x | 混合路由换 warp |
| 旋转 5° | 0.67x | > 1.5x | 增强 + 冗余 |
| 旋转 10° | 1.96x | > 3.0x | 增强 + 冗余 |
| JPEG q20 | 0.43x | > 1.0x | 去低频降噪 + 增强 |
| blur r5 | -0.82x | > 1.0x | 去低频降噪 + 增强 |
| 裁剪 75% | -0.24x | > 1.0x | 5 块冗余 |
