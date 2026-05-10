# 实现任务

## 任务依赖关系

```
Step 0: 清理编译警告（无依赖）
  ↓
Step 1: 添加 imageproc 依赖 + 实现 ORB 对齐（依赖 Step 0）
  ↓
Step 2: 增强 AlignmentResult / 对齐失败处理（依赖 Step 1）
  ↓
Step 3: 添加 JPEG 鲁棒性测试（依赖 Step 1）
  ↓
Step 4: 添加缩放对齐增强（依赖 Step 1）
  ↓
Step 5: 最终清理和定型（依赖 0-4 全部完成）
```

## Task 0: 清理编译警告

清理当前 8 个 warning：

- [x] 0.1 替换 `image::io::Reader` 为 `image::ImageReader`
- [x] 0.2 删除 `align.rs` 中未使用的变量（`gray_orig`, `gray_suspect`, `w`, `h`）
- [x] 0.3 标记 `align.rs` 中待用的函数 `rotate_rgb` 为 `#[allow(dead_code)]`
- [x] 0.4 修复 `align.rs` 多余括号 warning
- [x] 0.5 修复测试中未使用变量 `watermarked`
- [x] 0.6 确认 `cargo fmt --check` 通过

## Task 1: 实现 ORB 旋转对齐

核心工作：用 ORB 特征匹配替换当前的假对齐。

### 1.1 添加依赖

- [x] 1.1 在 `Cargo.toml` 中添加 `imageproc = "0.25"`

### 1.2 修改 AlignmentResult

- [x] 1.2 扩展 `AlignmentResult`，增加 `confidence: f64` 字段（0.0-1.0）

### 1.3 实现 ORB 对齐函数

- [x] 1.3 在 `align.rs` 中实现 `align_with_orb()` 函数：
  - 用 imageproc 的 oriented_fast 提取原图和怀疑图的关键点
  - 用 brief 计算描述子，match_binary_descriptors 做匹配
  - 用 RANSAC 筛选内点并估计单应性矩阵
  - 从单应性矩阵分解出旋转角度、缩放比例
  - 返回 `AlignmentResult`（含 rotation, scale, confidence）
- [x] 1.4 当特征点不够或内点比例太低时，confidence 设低值，fallback 到模板匹配
- [x] 1.5 更新 `align_images` 函数，集成 ORB 对齐 + 旧模板匹配作为 fallback
- [x] 1.6 确认检测流程正确使用旋转/缩放信息（warp 变换）

### 1.4 测试 ORB 对齐

- [x] 1.7 编写单元测试确保 ORB 对齐不崩溃
- [ ] 1.8 验证对齐 confidence 在正确匹配时高、错误匹配时低（在真实图像上验证）

## Task 2: 对齐失败处理

- [x] 2.1 修改 `detect_watermark` 逻辑：当 `alignment.confidence < 0.2` 时，直接返回 `detected: false` + 保留 alignment 信息
- [x] 2.2 在 CLI 输出中对齐 confidence 低时显示警告

## Task 3: 添加 JPEG 鲁棒性测试

- [x] 3.1 添加 JPEG 编码→解码测试（q90）
- [x] 3.2 JPEG q90 重压缩后检测测试
- [x] 3.3 JPEG q75 重压缩后检测测试
- [x] 3.4 JPEG q50 重压缩后检测测试（可放宽通过标准，用于信息记录）
- [x] 3.5 添加旋转 2°/5° 后检测测试（信息记录）
- [x] 3.6 添加缩放 90% 后检测测试（信息记录）

## Task 4: 缩放对齐增强

当前 `resize_rgb` 是简单最近邻采样。在 imageproc 支持下可以做得更好：

- [x] 4.1 利用 ORB 匹配得到的缩放比例直接恢复缩放的图片（通过 warp + resize）
- [x] 4.2 ORB 失败时 fallback 到 resize（已由 `fallback_result` 处理）

## Task 5: 最终清理和定型

- [x] 5.1 确认所有测试通过（12/12 passed）
- [x] 5.2 确认 `cargo clippy` 通过（0 errors, 1 minor warning）
- [x] 5.3 确认 `cargo fmt --check` 通过
- [x] 5.4 检查 Cargo.toml 依赖，确认无多余依赖
- [x] 5.5 检查 public API 一致（AlignmentResult 新增 confidence/scale 字段）
- [x] 5.6 更新 README 反映 non-blind 定位
