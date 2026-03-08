#!/bin/bash
# 自动化测试简拼算法脚本

echo "--- 正在编译项目 ---"
cargo build --quiet

echo "--- 开始测试简拼匹配 (nh -> 你好) ---"
# 使用 printf 模拟输入，通过 --test 模式运行，并检查输出
output=$(printf "nh\nexit\n" | cargo run --quiet -- --test)

if echo "$output" | grep -q "你好"; then
    echo "✅ 测试通过: 'nh' 匹配到了 '你好'"
else
    echo "❌ 测试失败: 'nh' 未匹配到 '你好'"
    echo "输出详情:"
    echo "$output"
fi

echo ""
echo "--- 开始测试全拼匹配 (zhao -> 赵/找) ---"
output=$(printf "zhao\nexit\n" | cargo run --quiet -- --test)

if echo "$output" | grep -qE "赵|找"; then
    echo "✅ 测试通过: 'zhao' 匹配到了预期汉字"
else
    echo "❌ 测试失败: 'zhao' 未匹配到预期汉字"
fi
