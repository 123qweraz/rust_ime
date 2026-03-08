import os
import subprocess

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
    candidates = []
    selected = 0
    page = 0
    total = 0
    
    # 获取最后一次状态
    last_block_start = 0
    for i in range(len(lines)-1, -1, -1):
        if "预编辑:" in lines[i]:
            last_block_start = i
            break
            
    relevant_lines = lines[last_block_start:]
    for line in relevant_lines:
        if "当前选中:" in line:
            selected = int(line.split(":")[1].strip())
        if "分页:" in line:
            parts = line.split(":")[1].strip().split("/")
            page = int(parts[0])
            total = int(parts[1])
        if "." in line and line.strip()[0].isdigit() and ". " in line:
            name = line.split(".")[1].split("(")[0].strip()
            candidates.append(name)
            
    return {"candidates": candidates, "selected": selected, "page": page, "total": total}

if __name__ == "__main__":
    print("--- 方向键功能逻辑测试 ---")
    
    # 1. DOWN 键测试 (当前实现预期为选词，修复后预期为翻页)
    print("\n[测试 1] 验证 DOWN 键行为...")
    res1_init = run_ime_cmd(["nihao"])
    print(f"初始状态: Page={res1_init['page']}, Selected={res1_init['selected']}")
    
    res1_down = run_ime_cmd(["nihao", "DOWN"])
    print(f"按下 DOWN 后: Page={res1_down['page']}, Selected={res1_down['selected']}")
    
    # 当前逻辑下，DOWN 会让 selected 变 1，而 page 不变
    # 修复后，DOWN 应该让 page 增加，selected 重置或按页移动
    if res1_down['page'] > res1_init['page']:
        print("✅ [预期] DOWN 键触发了翻页")
    elif res1_down['selected'] > res1_init['selected']:
        print("❌ [发现 BUG] DOWN 键表现为选择下一个词")
    else:
        print("! 状态未发生显著变化 (可能候选词太少)")

    # 2. RIGHT 键测试 (预期为选词)
    print("\n[测试 2] 验证 RIGHT 键行为...")
    res2_right = run_ime_cmd(["nihao", "RIGHT"])
    print(f"按下 RIGHT 后: Page={res2_right['page']}, Selected={res2_right['selected']}")
    if res2_right['selected'] > res1_init['selected']:
         print("✅ [预期] RIGHT 键触发了选择下一个词")
    else:
         print("❌ [失败] RIGHT 键未按预期选词")
