
import subprocess
import json
import time

def run_test():
    # 模拟输入 'de;rp' 看看是否会自动上屏 '的'
    # 注意：我们的处理器逻辑在 stroke 方案下只要候选词唯一就会自动上屏
    # 我们先测试辅助码模式下的自动上屏 (de;rp)
    
    print("--- 测试笔画辅助码自动上屏 (de;rp -> 的) ---")
    
    # 模拟按键序列
    keys = ["d", "e", "semicolon", "r", "p"]
    
    cmd = ["cargo", "run", "--bin", "rust-ime", "--", "--test-keys"]
    cmd.extend(keys)
    
    result = subprocess.run(cmd, capture_output=True, text=True)
    output = result.stdout
    
    print(f"Output: {output}")
    
    if "Commit: 的" in output or "Commit:的" in output:
        print("✅ [通过] 笔画辅助码唯一候选词自动上屏")
    else:
        print("❌ [失败] 笔画辅助码未自动上屏")

    print("\n--- 测试独立笔画方案自动上屏 (Switch to stroke -> rp -> 的) ---")
    # 模拟切换到笔画方案 (Grave `) 然后输入 rp
    keys = ["grave", "r", "p"]
    
    cmd = ["cargo", "run", "--bin", "rust-ime", "--", "--test-keys"]
    cmd.extend(keys)
    
    result = subprocess.run(cmd, capture_output=True, text=True)
    output = result.stdout
    print(f"Output: {output}")
    
    if "Commit: 的" in output or "Commit:的" in output:
        print("✅ [通过] 独立笔画方案唯一候选词自动上屏")
    else:
        # 如果 rp 不唯一，可能需要更多的键，但在我们的词典里 rp 应该是唯一的
        print("❌ [失败] 独立笔画方案未自动上屏")

if __name__ == "__main__":
    run_test()
