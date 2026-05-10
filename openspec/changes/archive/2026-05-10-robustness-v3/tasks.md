# 实现任务

## 执行顺序

```
Task 1: 去除低频段（回退 E）
  ↓
Task 2: 混合对齐策略（升级 C）
  ↓
Task 3: 增强信号 + 十字冗余（升级 D）
  ↓
Task 4: 摸底验证
```

## Task 1: 去除低频段

- [x] 1.1 在 `embed_watermark` 中删除低频段嵌入代码，只保留中频段
- [x] 1.2 在 `detect_watermark` 传统路径中删除低频段 score 计算
- [x] 1.3 在 `detect_via_prn_correction` 中删除低频段 score 和 noise 计算
- [x] 1.4 删除 `get_low_freq_positions`、`get_high_freq_positions` 及相关常量
- [x] 1.5 中频嵌入强度因子从 1.0 调至 1.5
- [x] 1.6 `cargo build` 通过

- [x] 2.1 添加路由：`|rotation| ≤ 3.0 && |scale - 1.0| < 0.05` → warp
- [x] 2.2 提取现有 warp 路径为 `detect_via_warp()` 函数
- [x] 2.3 当 ORB transform 为 None 时，走 warp 路径
- [x] 2.4 `cargo build` 通过

- [x] 3.1 PRN 组合从 2 邻居扩展为 4 邻居（左/上/右/下）
- [x] 3.2 边缘邻居不存在时 PRN 置 0
- [x] 3.3 warp 检测路径 PRN 组合扩展为 4 邻居
- [x] 3.4 PRN 校正路径 PRN 组合扩展为 4 邻居
- [x] 3.5 `cargo build` 通过

## Task 4: 验证

- [x] 4.1 运行 `cargo test --test robust -- --test-threads=1`
- [x] 4.2 确认所有 13 个测试通过
- [x] 4.3 查看摸底调查数据，确认目标达成
