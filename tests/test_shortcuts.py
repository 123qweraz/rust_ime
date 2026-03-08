import os
import subprocess
import shutil

# 确保在项目根目录运行
os.chdir(os.path.dirname(os.path.abspath(__file__)) + "/..")

def run_ime_cmd(inputs):
    process = subprocess.Popen(
        ["./target/debug/rust-ime", "--test"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    full_input = "\n".join(inputs) + "\nexit\n"
    out, err = process.communicate(input=full_input)
    
    # 提取最后一次查询的结果
    lines = out.splitlines()
    last_action = ""
    buffer_content = ""
    
    for line in reversed(lines):
        if "动作反馈:" in line and not last_action:
            last_action = line.split(":")[1].strip()
        if "原始缓冲区:" in line and not buffer_content:
            buffer_content = line.split(":")[1].strip()
            
    return {"action": last_action, "buffer": buffer_content, "full": out}

if __name__ == "__main__":
    print("--- 系统快捷键透传集成测试 ---")
    
    # 1. 测试 Ctrl + C (应透传，不进 buffer)
    print("\n[测试 1] 验证 Ctrl + C 透传...")
    res1 = run_ime_cmd(["CTRL_C"])
    print(f"动作反馈: {res1['action']}")
    print(f"缓冲区: '{res1['buffer']}'")
    
    if "PassThrough" in res1['action'] and not res1['buffer']:
        print("✅ [成功] Ctrl + C 已正确透传")
    else:
        print("❌ [失败] Ctrl + C 被拦截或进入了缓冲区")

    # 2. 测试 Ctrl + V
    print("\n[测试 2] 验证 Ctrl + V 透传...")
    res2 = run_ime_cmd(["CTRL_V"])
    print(f"动作反馈: {res2['action']}")
    if "PassThrough" in res2['action']:
        print("✅ [成功] Ctrl + V 已正确透传")
    else:
        print("❌ [失败] Ctrl + V 未能透传")

    # 3. 测试 Alt + F (应透传)
    print("\n[测试 3] 验证 Alt + F 透传...")
    res3 = run_ime_cmd(["ALT_F"])
    print(f"动作反馈: {res3['action']}")
    if "PassThrough" in res3['action']:
        print("✅ [成功] Alt + F 已正确透传")
    else:
        print("❌ [失败] Alt + F 未能透传")

    # 4. 验证正常输入不受影响 (无修饰键)
    print("\n[测试 4] 验证正常输入逻辑...")
    res4 = run_ime_cmd(["a"])
    print(f"缓冲区: '{res4['buffer']}'")
    if res4['buffer'] == "a":
        print("✅ [成功] 正常字母输入逻辑依然稳健")
    else:
        print("❌ [失败] 正常输入逻辑被破坏")
