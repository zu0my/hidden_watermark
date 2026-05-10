# 精简检测管线：切除历史绕路

## 问题

经过 17 轮迭代后，代码中残留了多处历史绕路的痕迹。每一轮都在现有结构上"叠加"新策略，前一轮被认定为错误的部分从未被彻底切除：

1. **低频段嵌入未清除**：`robustness-v3` 已认定低频段稀释信号、降低干净图像 ratio 从 26x 到 2.09x，明确要求"删除多频段 (E) 的嵌入和检测代码"。但 `embed_watermark` 仍在嵌入低频段，`detect_via_prn_correction` 仍在检测低频段，而 `detect_via_warp` 不检测——三个函数对低频段的态度不一致。

2. **代码重复**：十字形 PRN 组合（`own + left + top + right + bottom`）在 3 个位置重复展开；面积加权角点映射在 `detect_via_prn_correction` 中出现了两次（score 计算 + noise 估计）。

3. **死代码**：`transform_block_coord` 被公开导出但无人调用，PRN 校正函数自己内联了同样的逻辑。

## 方案

三步精简，每步独立可验证：

1. **切除低频段**：从嵌入、PRN 校正检测、公共 API 中完全移除 `low_freq` 相关代码。PRN key 去 `_mid` 后缀。
2. **消除重复**：提取 `combined_prn(a, b, neighbors)` 和 `prn_correction_weights(suspect_block, transform)` 公共函数。
3. **清理死代码**：移除 `transform_block_coord`，删除未使用的 `lazy_static` 依赖。

## 非目标

- 不改变双路由架构（`unified-warp-test` 已验证需保留）
- 不改变嵌入强度、阈值等参数
- 不修改对齐逻辑
- 不增加新依赖

## 预期

| 指标 | 当前 | 目标 |
|------|------|------|
| 代码行数 (lib.rs) | 739 | < 600 |
| 测试通过 | 13/13 | 13/13 |
| 干净图像 ratio | 当前值 | 不退化 |
| 各攻击场景 ratio | 当前值 | 不退化 |
