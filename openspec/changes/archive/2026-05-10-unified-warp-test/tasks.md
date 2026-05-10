# 实现任务

## Task 1: 切换路由

- [x] 1.1 在 `detect_watermark` 中删除路由逻辑，统一调用 `detect_via_warp`
- [x] 1.2 确认 `cargo build` 通过

## Task 2: 摸底验证

- [x] 2.1 运行摸底测试（多轮迭代）
- [x] 2.2 分析数据：最优配置为 0.5 中心 + 7° 阈值 + 中频 2.0 + 低频 0.7

## Task 3: 最终决策

- [x] 3.1 决策：保留路由（warp ≤7° / PRN 校正 >7°），85% 攻击类型通过
- [x] 3.2 保留 PRN 校正和双频段嵌入（对大角度旋转和缩放有价值）
- [x] 3.3 `cargo test --test robust -- --test-threads=1` 13/13 通过
