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
    
    # 提取所有反馈动作
    lines = out.splitlines()
    actions = []
    
    for line in lines:
        if "动作反馈:" in line:
            actions.append(line.split(":")[1].strip())
            
    return actions

if __name__ == "__main__":
    print("--- 系统快捷键生命周期测试 (防止按键粘连) ---")
    
    # 1. 测试 Ctrl + C 的完整生命周期
    # 序列: 按下 CTRL_C, 释放 UP_CTRL_C
    print("\n[测试 1] 验证 Ctrl + C 的按下与释放...")
    actions1 = run_ime_cmd(["CTRL_C", "UP_CTRL_C"])
    print(f"动作序列: {actions1}")
    
    if len(actions1) >= 2 and all("PassThrough" in a for a in actions1):
        print("✅ [成功] Ctrl + C 的所有事件均已正确透传")
    else:
        print("❌ [失败] 部分事件被拦截，可能导致按键粘连")

    # 2. 测试在 Buffer 不为空时使用快捷键 (这是最容易出问题的场景)
    print("\n[测试 2] 验证 Buffer 不为空时快捷键的透传...")
    # 序列: w, o, CTRL_C, UP_CTRL_C
    actions2 = run_ime_cmd(["w", "o", "CTRL_C", "UP_CTRL_C"])
    # 动作反馈顺序: Emit(w), Emit(o), CTRL_C反馈, UP_CTRL_C反馈
    shortcut_actions = actions2[2:] if len(actions2) > 2 else []
    print(f"快捷键反馈: {shortcut_actions}")
    
    if shortcut_actions and all("PassThrough" in a for a in shortcut_actions):
        print("✅ [成功] Buffer 占用时，快捷键释放事件依然正确透传")
    else:
        print("❌ [失败] Buffer 占用导致释放事件被拦截")

    # 3. 验证修饰键本身 (单独按 Ctrl)
    # 我们暂不支持单独 CTRL 映射，但可以确保它不被 Consume
    print("\n[测试 3] 验证单独 Ctrl 键释放...")
    # 注意: 需要在 main.rs 增加对单独 CTRL 支持才能测这个，目前我们支持 CTRL_字母
    # 我们先用 CTRL_C 的释放来代表
