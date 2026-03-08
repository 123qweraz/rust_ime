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
    
    lines = out.splitlines()
    status = {"preedit": "", "buffer": "", "filter_mode": "", "aux_filter": ""}
    
    # 逆序寻找最后的状态报告
    for line in reversed(lines):
        if "预编辑:" in line: status["preedit"] = line.split(":")[1].strip(); break
    for line in reversed(lines):
        if "原始缓冲区:" in line: status["buffer"] = line.split(":")[1].strip(); break
    for line in reversed(lines):
        if "过滤模式:" in line: status["filter_mode"] = line.split(":")[1].strip(); break
    for line in reversed(lines):
        if "辅助码过滤:" in line: status["aux_filter"] = line.split(":")[1].strip(); break
            
    return status

if __name__ == "__main__":
    print("--- 大写字母输入与英文过滤系统集成测试 ---")
    
    # 1. 测试 Shift + 字母 (应保持大小写，且不误触过滤)
    print("\n[测试 1] 验证 Shift + 字母输入...")
    res1 = run_ime_cmd(["SHIFT_W", "o"])
    print(f"原始缓冲区: {res1['buffer']}")
    print(f"预编辑显示: {res1['preedit']}")
    print(f"当前过滤模式: {res1['filter_mode']}")
    
    if "Wo" in res1['buffer'] and "Wo" in res1['preedit'] and "None" in res1['filter_mode']:
        print("✅ [成功] 大写字母正确进入 Buffer 且显示正确，未误触发过滤")
    else:
        print("❌ [失败] 输入大小写或过滤模式异常")

    # 2. 测试 CapsLock 状态
    print("\n[测试 2] 验证 CapsLock 锁定状态下的输入...")
    # 序列: CAPSLOCK (按下), W, o (此时 CapsLock 依然开启)
    res2 = run_ime_cmd(["CAPSLOCK", "W", "O"])
    print(f"原始缓冲区: {res2['buffer']}")
    if "WO" in res2['buffer']:
        print("✅ [成功] CapsLock 状态下的字符映射逻辑正确")
    else:
        print("❌ [失败] CapsLock 下字符错误")

    # 3. 验证单独 Shift 触发全局过滤
    print("\n[测试 3] 验证单独 Shift 在缓冲区不为空时触发全局过滤...")
    # 序列: w, o, SHIFT
    res3 = run_ime_cmd(["w", "o", "SHIFT"])
    print(f"最终过滤模式: {res3['filter_mode']}")
    if "Global" in res3['filter_mode']:
        print("✅ [成功] 单独释放 Shift 成功触发了全局过滤模式")
    else:
        print("❌ [失败] 未能进入全局过滤模式")
