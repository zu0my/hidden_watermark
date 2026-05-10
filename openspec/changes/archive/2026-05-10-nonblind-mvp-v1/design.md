# Non-blind MVP v1 技术方案

## 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI (main.rs)                        │
│  embed │ detect │ detect-batch                               │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                    lib.rs (核心逻辑)                         │
│                                                             │
│  embed_watermark(image, key, strength) → (RgbImage, f64)     │
│    1. crop_to_multiple                                       │
│    2. RGB→Y (只改亮度通道)                                   │
│    3. 分块 DCT → 中频嵌入 PRN 序列                           │
│    4. Y→RGB → PSNR 计算                                     │
│                                                             │
│  detect_watermark(original, suspect, key, fpr) → DetectionResult │
│    1. align_images (含旋转/缩放/平移)                         │
│    2. normalize_histogram                                    │
│    3. 分块 DCT → 差分 → 相关性统计                            │
│    4. 阈值判定 → DetectionResult                             │
└──────────────────────┬──────────────────────────────────────┘
                       │
          ┌────────────┼────────────┐
          ▼            ▼            ▼
      midfreq.rs   align.rs     [imageproc]
      ──────────   ──────────   ──────────
      DCT 16x16    ORB 特征匹配  ORB 特征提取
      PRN 生成     模板匹配      描述子匹配
      中频位置     直方图归一化  单应性矩阵估计
      纹理权重     旋转/缩放
