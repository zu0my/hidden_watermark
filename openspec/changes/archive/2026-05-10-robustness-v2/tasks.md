# 实现任务

## 执行顺序

```
Task A: 检测时只使用中心区域 (独立，第一个做)
  ↓
Task C: 实现 PRN 域校正 (核心，依赖 A 的结构)
  ↓
Task D: 实现块冗余嵌入 (修改嵌入 + 检测，依赖 A+C 的检测逻辑)
  ↓
Task E: 实现多频段嵌入 (修改嵌入 + 检测，依赖 A+C+D)
  ↓
Task V: 摸底测试验证 (验证四层组合效果)
```

## Task A: 中心区域检测（A）

- [x] A.1 在 `lib.rs` 中添加 `center_blocks_region()` 辅助函数，返回中心区域的块范围
- [x] A.2 修改 score 计算循环，只遍历中心区域的块
- [x] A.3 修改 threshold/noise 计算循环，只使用中心区域的块系数
- [x] A.4 确认 PSNR 测试不受影响（嵌入不变）

## Task C: PRN 域校正（C）

- [x] C.1 在 `align.rs` 中添加 `transform_block_coord()` 函数和 `transform` 字段
- [x] C.2 在 `align.rs` 中添加块映射逻辑（双线性加权）
- [x] C.3 修改 `detect_watermark` 检测逻辑：当 ORB 可用时走 PRN 校正路径，不做 warp
- [x] C.4 当 PRN 校正路径不可用（fallback），回退到 warp + 检测逻辑

## Task D: 块冗余嵌入（D）

- [x] D.1 修改 `embed_watermark`：每个块嵌入时叠加左/上邻居的 PRN（强度 0.5）
- [x] D.2 修改检测逻辑：传统路径和 PRN 校正路径均使用相同的未加权组合 PRN
- [x] D.3 处理图像边缘：左/上邻居不存在时只嵌入可用的 PRN

## Task E: 多频段嵌入（E）- 适度版本

- [x] E.1 在 `midfreq.rs` 中定义低/中频段范围
- [x] E.2 修改 `embed_watermark`：在低/中频段独立嵌入 PRN 序列
- [x] E.3 修改检测逻辑：两个频段独立做相关，等权重合并
- [x] E.4 确保 PSNR 仍 > 40 dB

## Task V: 摸底测试验证

- [x] V.1 运行摸底测试 `test_robustness_survey` 收集完整数据
- [x] V.2 对比 baseline 和 v2 的各攻击 ratio
- [x] V.3 确认所有原有 12 个测试通过
