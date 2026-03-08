#!/bin/bash
# 自动化测试辅助码过滤功能 (粘性 Shift 释放测试)

echo "--- 正在编译项目 ---"
cargo build --quiet

echo "--- 开始测试粘性辅助码过滤 (ma -> [Shift+C] -> o -> d -> e -> 码) ---"
# 模拟按键序列：
# m, a (输入拼音)
# SHIFT_C (组合键触发过滤)
# o, d, e (松开 Shift 后继续输入后续字母)
output=$(printf "m\na\nSHIFT_C\no\nd\ne\nexit\n" | cargo run --quiet -- --test)

# 检查最终是否通过连续过滤锁定了 '码' 并自动上屏
if echo "$output" | grep -q "DeleteAndEmit.*insert: \"码\""; then
    echo "✅ 测试通过: 'ma' + Shift+C (松开) + ode 成功锁定并上屏了 '码'"
else
    echo "❌ 测试失败: 连续过滤逻辑仍有误"
    echo "输出详情 (过滤过程):"
    echo "$output" | grep "辅助码过滤"
fi
