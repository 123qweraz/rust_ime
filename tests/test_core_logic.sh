#!/bin/bash
# 核心逻辑集成测试脚本 - 严格校验 Action 反馈

# 确保在项目根目录运行
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
cd "$SCRIPT_DIR/.."

echo "--- 正在编译项目 ---"
cargo build --quiet

# 辅助函数：运行测试并校验结果
# 参数1: 模拟输入 (e.g. "n\ni\nh\na\no\n ")
# 参数2: 期望在 Action 反馈中看到的关键词 (e.g. "你好")
# 参数3: 测试名称
run_test() {
    local input=$1
    local expected=$2
    local name=$3
    
    # 模拟输入流，最后加一个 exit 退出 REPL
    output=$(printf "${input}\nexit\n" | cargo run --quiet -- --test)
    
    if echo "$output" | grep -q "动作反馈:.*${expected}"; then
        echo "✅ [通过] $name"
    else
        echo "❌ [失败] $name"
        echo "   期望 Action 包含: $expected"
        echo "   输出详情 (Action 部分):"
        echo "$output" | grep "动作反馈"
        exit 1
    fi
}

echo "--- 开始核心逻辑回归测试 ---"
# 清理可能存在的历史数据，确保测试环境纯净
rm -rf data/user_data.db

# 测试 1: 空格键上屏汉字 (校验 Action)
# 输入 'n', 'i', 'h', 'a', 'o', ' ' (空格)
run_test "n\ni\nh\na\no\n " "你好" "全拼+空格上屏汉字"

# 测试 2: 辅助码连续锁定并自动上屏
# 输入 'm', 'a', 'SHIFT_C', 'o', 'd', 'e'
run_test "m\na\nSHIFT_C\no\nd\ne" "码" "辅助码连续过滤并自动上屏"

# 测试 3: 简拼匹配
run_test "n\nh\n " "孩" "简拼匹配校验"

# 测试 4: 方向键逻辑 (Down 翻页, Right 选词)
# 初始输入 'nihao'，然后按 DOWN，期望 Page 发生变化
# 这里我们通过检查输出中的 "分页: 5" 来验证 (假设每页 5 个)
output_down=$(printf "nihao\nDOWN\nexit\n" | cargo run --quiet -- --test)
if echo "$output_down" | grep -q "分页: [1-9]"; then
    echo "✅ [通过] DOWN 键翻页逻辑"
else
    echo "❌ [失败] DOWN 键翻页逻辑"
    echo "$output_down" | grep "分页"
    exit 1
fi

output_right=$(printf "nihao\nRIGHT\nexit\n" | cargo run --quiet -- --test)
if echo "$output_right" | grep -q "当前选中: 1"; then
    echo "✅ [通过] RIGHT 键选词逻辑"
else
    echo "❌ [失败] RIGHT 键选词逻辑"
    echo "$output_right" | grep "当前选中"
    exit 1
fi

echo "--- 所有集成测试已通过！ ---"
