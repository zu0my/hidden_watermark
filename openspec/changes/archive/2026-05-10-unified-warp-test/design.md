# 统一 Warp 路径 - 技术方案

## 改动

**唯一代码改动**：`src/lib.rs` 中的 `detect_watermark` 函数。

```diff
- // Routing: warp for clean/non-geometric, PRN correction for geometric
- let has_transform = alignment.transform.is_some();
- let angle_deg = alignment.rotation.abs();
- let scale_diff = (alignment.scale - 1.0).abs();
- let is_geometric = angle_deg > 0.5 || scale_diff > 0.02;
- 
- if has_transform && is_geometric {
-     return detect_via_prn_correction(&original, &suspect, key, fpr, alignment);
- }
- 
- detect_via_warp(&original, &aligned, key, fpr, alignment)
+ // Unified warp path: align_images already corrected geometry
+ detect_via_warp(&original, &aligned, key, fpr, alignment)
```

`detect_via_prn_correction` 函数保留但不被调用（验证阶段不删代码）。

## 原理

`align_images` 已经通过 ORB + warp 将 suspect 旋转/缩放回 original 的网格。`detect_via_warp` 在这个对齐后的图像上做 DCT 比对。信号越强，warp 的插值误差影响越小。v3 信号是 baseline 的 1.4 倍——需要验证这个余量是否足以覆盖所有旋转角度。

## 验证

- 运行 `test_robustness_survey` 摸底测试，搜集完整数据
- 确认 13 个测试全部通过
