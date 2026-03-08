import os
import subprocess
import json
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
    status = {"chinese": True, "selected": 0, "page": 0, "profiles": []}
    
    # 逆序寻找最后的状态报告
    for line in reversed(lines):
        if "中英文状态:" in line:
            status["chinese"] = "开启" in line
            break # 只要最新的
            
    for line in reversed(lines):
        if "当前选中:" in line:
            status["selected"] = int(line.split(":")[1].strip())
            break
            
    return status

def update_config(config_path, updates):
    if os.path.exists(config_path):
        with open(config_path, 'r') as f:
            cfg = json.load(f)
    else:
        # 如果文件不存在，至少提供一个合法的 Hotkeys 结构骨架
        cfg = {
            "switch_language": {"key": "tab", "description": "核心: 切换中/英文模式"},
            "page_up": ["Up", "PageUp", "-", ",", "["],
            "page_down": ["Down", "PageDown", "=", ".", "]"],
            "prev_candidate": ["Left"],
            "next_candidate": ["Right"],
            "enable_tab_toggle": True,
            "enable_ctrl_space_toggle": False
        }
    
    cfg.update(updates)
    with open(config_path, 'w') as f:
        json.dump(cfg, f)

if __name__ == "__main__":
    config_path = "configs/hotkeys.json"
    backup_path = "configs/hotkeys.json.bak"
    
    print("--- 快捷键系统全量逻辑测试 ---")
    if os.path.exists(config_path):
        shutil.copy(config_path, backup_path)
    
    try:
        # 1. 测试 Ctrl + Space 切换
        print("\n[测试 1] 验证 Ctrl + Space 切换中英文...")
        update_config(config_path, {"enable_ctrl_space_toggle": True, "enable_tab_toggle": False})
        
        res1 = run_ime_cmd(["CTRL_SPACE"])
        print(f"执行后状态: {'开启' if res1['chinese'] else '关闭'}")
        if not res1['chinese']:
            print("✅ [成功] Ctrl + Space 成功切换至英文")
        else:
            print("❌ [失败] Ctrl + Space 未能切换")

        # 2. 测试 Tab 切换开关
        print("\n[测试 2] 验证 Tab 切换开关...")
        # 开启 Tab 切换
        update_config(config_path, {"enable_ctrl_space_toggle": False, "enable_tab_toggle": True})
        res2_on = run_ime_cmd(["TAB"])
        
        # 关闭 Tab 切换
        update_config(config_path, {"enable_ctrl_space_toggle": False, "enable_tab_toggle": False})
        res2_off = run_ime_cmd(["TAB"])
        
        if not res2_on['chinese'] and res2_off['chinese']:
            print("✅ [成功] Tab 切换开关控制逻辑正确")
        else:
            print(f"❌ [失败] Tab 切换逻辑异常: On={'开启' if res2_on['chinese'] else '关闭'}, Off={'开启' if res2_off['chinese'] else '关闭'}")

        # 3. 测试 CapsLock 方案切换
        print("\n[测试 3] 验证 CapsLock 方案快速切换...")
        # 预设一个映射: 字母 'C' -> 'chinese' (假设系统默认有这个)
        # 我们这里通过拦截动作 Consume 来确认逻辑已触发
        process = subprocess.Popen(["./target/debug/rust-ime", "--test"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        out, _ = process.communicate(input="CAPSLOCK\nC\nexit\n")
        if "动作反馈: Consume" in out:
             print("✅ [成功] CapsLock -> 方案键 序列处理逻辑正确")
        else:
             print("❌ [失败] CapsLock 方案切换逻辑未按预期执行")

    finally:
        if os.path.exists(backup_path):
            shutil.move(backup_path, config_path)
        print("\n配置已恢复。")
