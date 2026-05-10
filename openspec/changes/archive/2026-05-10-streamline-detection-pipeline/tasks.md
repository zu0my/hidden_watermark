# 实现任务

## Task 1: 基线验证

- [x] 1.1 运行 `cargo test --test robust -- --test-threads=1`，记录 13 个测试结果
- [x] 1.2 运行摸底测试 `test_robustness_survey`，记录所有攻击场景 ratio 作为 baseline

## Task 2: 提取 combined_prn 公共函数

- [x] 2.1 在 `src/midfreq.rs` 中新增 `combined_prn` 函数
- [x] 2.2 替换 `embed_watermark` 中 `embed_cross` 闭包内的 PRN 组合逻辑
- [x] 2.3 替换 `detect_via_warp` 中 block score 计算的 PRN 组合逻辑
- [x] 2.4 替换 `detect_via_prn_correction` 中 `band_compute` 闭包内的 PRN 组合逻辑
- [x] 2.5 运行 `cargo test --test robust -- --test-threads=1`，确认 13/13 通过

## Task 3: 切除低频段

- [x] 3.1 从 `embed_watermark` 中移除 `low_freq` 嵌入代码，保留 `_mid` 后缀
- [x] 3.2 从 `detect_via_prn_correction` 中移除 `low_freq` 检测代码
- [x] 3.3 从 `lib.rs` 的 `pub use` 中移除 `get_low_freq_positions`
- [x] 3.4 运行 `cargo build` 确认编译通过
- [x] 3.5 运行 `cargo test --test robust -- --test-threads=1`，确认 13/13 通过
- [x] 3.6 对比摸底测试数据，确认无退化

## Task 4: 消除 PRN 校正中的重复

- [x] 4.1 提取 `map_corners_to_weights` 私有函数
- [x] 4.2 用此函数替换 score 计算和 noise 估计中的两处重复代码
- [x] 4.3 运行 `cargo test --test robust -- --test-threads=1`，确认 13/13 通过

## Task 5: 清理死代码

- [x] 5.1 从 `src/align.rs` 删除 `transform_block_coord` 函数
- [x] 5.2 从 `src/lib.rs` 的 `pub use` 中移除 `transform_block_coord`
- [x] 5.3 检查 `lazy_static` 是否仍被使用，若否，从 `Cargo.toml` 移除
- [x] 5.4 运行 `cargo build && cargo test --test robust -- --test-threads=1`，确认全部通过

## Task 6: 最终验证

- [x] 6.1 运行 `cargo clippy -- -D warnings` 确认无警告
- [x] 6.2 运行 `cargo test -- --test-threads=1` 确认全部通过
- [x] 6.3 运行摸底测试，对比 baseline，确认所有 ratio 不退化
- [x] 6.4 运行 `cargo build --release` 确认 release 编译通过

## 实现说明

- PRN key 保留了 `_mid` 后缀：移除后缀会改变 PRN 序列，导致旋转和缩放检测退化，已验证回退
- 低频段常量和函数从 `midfreq.rs` 完全移除（无测试引用）
- 添加了 `noise_levels.is_empty()` 防护，修复了 blur r7 场景下的潜在 panic
