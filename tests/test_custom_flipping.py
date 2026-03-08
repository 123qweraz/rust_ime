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
    page = 0
    
    for line in lines:
        if "分页:" in line:
            page = int(line.split(":")[1].strip().split("/")[0])
            
    return page

if __name__ == "__main__":
    config_path = "configs/hotkeys.json"
    backup_path = "configs/hotkeys.json.bak"
    
    print("--- 自定义翻页键测试 (使用 ] 键) ---")
    
    # 1. 备份
    if os.path.exists(config_path):
        shutil.copy(config_path, backup_path)
    
    try:
        # 2. 修改配置: 将 page_down 设置为只有 ']'
        # 注意: 配置文件中存储的是字符串，根据 VirtualKey::from_str, ']' 映射到 RightBrace
        cfg = {}
        if os.path.exists(config_path):
            with open(config_path, 'r') as f:
                cfg = json.load(f)
        
        cfg['page_down'] = [']']
        if 'hotkeys' not in cfg: # 兼容全量配置结构
             cfg = {"hotkeys": cfg} if "switch_language" in cfg else cfg

        with open(config_path, 'w') as f:
            json.dump(cfg if "hotkeys" in cfg else {"hotkeys": cfg}, f)
            
        print("已将 page_down 设置为 [']']")

        # 3. 运行测试
        # 确认按 ] 会翻页
        page_brace = run_ime_cmd(["nihao", "]"])
        print(f"按下 ] 后的页码: {page_brace}")
        
        if page_brace > 0:
            print("✅ [成功] 自定义翻页键生效：] 成功翻页！")
        else:
            print("❌ [失败] 自定义翻页键 ] 未能翻页。")
            
    finally:
        # 4. 恢复
        if os.path.exists(backup_path):
            shutil.move(backup_path, config_path)
        print("配置已恢复。")
