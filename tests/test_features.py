import subprocess
import json
import time

def run_ime_command(commands):
    """运行 IME 并发送一系列指令，返回输出结果"""
    process = subprocess.Popen(
        ['target/debug/rust-ime', '--test'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    input_str = "\n".join(commands) + "\nexit\n"
    stdout, stderr = process.communicate(input=input_str, timeout=10)
    return stdout

def test_capslock_vim_navigation():
    print("测试: CapsLock VIM 导航 (HJKL)...")
    # 模拟输入 nihao, 按下 CapsLock (进入 nav_mode), 按 L (NextCandidate), 按空格上屏
    # 注意: --test 模式下 CAPSLOCK 模拟按下
    commands = [
        "n", "i", "h", "a", "o",
        "CAPSLOCK", # 进入导航模式
        "l",         # 下一个候选词 (L 映射)
        "UP_CAPSLOCK", # 释放导航模式
        " ",         # 上屏
    ]
    output = run_ime_command(commands)
    
    # 检查是否选中了第 2 个候选词 (拟好)
    if "动作反馈: DeleteAndEmit { delete: 5, insert: \"拟好\" }" in output or "拟好" in output:
        print("✅ CapsLock VIM 导航测试通过")
    else:
        print("❌ CapsLock VIM 导航测试失败")
        # print(output)

def test_capslock_profile_switch():
    print("测试: CapsLock 快捷切换方案...")
    # 假设配置中 'e' 映射到 english 方案
    # 输入 CapsLock + e
    commands = [
        "CAPSLOCK",
        "e",
        "UP_CAPSLOCK",
    ]
    output = run_ime_command(commands)
    
    if "已切换至英语方案" in output or "english" in output.lower():
        print("✅ CapsLock 方案切换测试通过")
    else:
        print("❌ CapsLock 方案切换测试失败")
        print("--- DEBUG OUTPUT ---")
        print(output)
        print("--------------------")

if __name__ == "__main__":
    try:
        # 确保项目已编译
        subprocess.run(["cargo", "build"], check=True)
        test_capslock_vim_navigation()
        test_capslock_profile_switch()
    except Exception as e:
        print(f"测试过程中发生错误: {e}")
