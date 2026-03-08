import subprocess
import json
import time
import re

def strip_ansi(text):
    """去除终端颜色代码"""
    return re.sub(r'\x1b\[[0-9;]*m', '', text)

def run_ime_command(commands):
    """运行 IME 并发送一系列指令，返回输出结果"""
    process = subprocess.Popen(
        ['target/debug/rust-ime', '--test'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    
    # 模拟真实按键间隔，防止状态竞争
    full_output = ""
    for cmd in commands:
        process.stdin.write(cmd + "\n")
        process.stdin.flush()
        time.sleep(0.1) 
    
    process.stdin.write("exit\n")
    process.stdin.flush()
    
    stdout, stderr = process.communicate(timeout=10)
    return strip_ansi(stdout)

def test_capslock_vim_navigation():
    print("测试: CapsLock VIM 导航 (HJKL)...")
    # 模拟输入 nihao, 按住 CapsLock, 按 L 选下一个, 再按 L 选下一个, 释放 CapsLock, 空格上屏
    commands = [
        "n", "i", "h", "a", "o",
        "CAPSLOCK", 
        "l",         # 下一个候选词 (选中 2: 拟好)
        "l",         # 下一个候选词 (选中 3: 倪浩)
        "UP_CAPSLOCK",
        " ",         # 上屏
    ]
    output = run_ime_command(commands)
    
    if "倪浩" in output:
        print("✅ CapsLock VIM 左右选词测试通过")
    else:
        print("❌ CapsLock VIM 左右选词测试失败")

    # 测试上下翻页
    print("测试: CapsLock VIM 上下翻页 (J/K)...")
    commands = [
        "a",         # 输入 a (很多候选词)
        "CAPSLOCK",
        "j",         # 下一页 (selected 应该变为 5 或更多)
        "UP_CAPSLOCK",
    ]
    output = run_ime_command(commands)
    if "分页: 5/" in output or "分页: 10/" in output:
        print("✅ CapsLock VIM 上下翻页测试通过")
    else:
        print("❌ CapsLock VIM 上下翻页测试失败")
        with open("page_flip_debug.txt", "w") as f:
            f.write(output)

def test_capslock_delayed_switch():
    print("测试: CapsLock 延迟切换方案 (先按Caps, 释放, 再按E)...")
    commands = [
        "CAPSLOCK",
        "UP_CAPSLOCK",
        "e",
    ]
    output = run_ime_command(commands)
    
    if "已切换至英语方案" in output or "english" in output.lower():
        print("✅ CapsLock 延迟切换方案测试通过")
    else:
        print("❌ CapsLock 延迟切换方案测试失败")

def test_capslock_shielding():
    print("测试: CapsLock 大写锁定屏蔽...")
    commands = [
        "CAPSLOCK",
        "UP_CAPSLOCK",
        "a",
    ]
    output = run_ime_command(commands)
    # 由于 CapsLock 大写锁定已屏蔽，按 a 应该还是小写 a
    if "原始缓冲区: a" in output:
        print("✅ CapsLock 大写锁定屏蔽测试通过")
    else:
        print("❌ CapsLock 大写锁定屏蔽测试失败 (可能切换了大写)")

if __name__ == "__main__":
    try:
        subprocess.run(["cargo", "build"], check=True)
        test_capslock_vim_navigation()
        test_capslock_delayed_switch()
        test_capslock_shielding()
    except Exception as e:
        print(f"测试过程中发生错误: {e}")
