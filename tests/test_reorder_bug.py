import os
import subprocess
import time

# 确保在项目根目录运行
os.chdir(os.path.dirname(os.path.abspath(__file__)) + "/..")

def run_ime_cmd(inputs):
    """运行 rust-ime 并模拟输入，返回最后的候选词列表"""
    process = subprocess.Popen(
        ["./target/debug/rust-ime", "--test"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    # 输入拼音，然后可能输入数字选择，最后 exit
    full_input = "\n".join(inputs) + "\nexit\n"
    out, err = process.communicate(input=full_input)
    
    candidates = []
    found_cand = False
    for line in out.splitlines():
        if "候选词" in line:
            found_cand = True
            candidates = [] # 重置，只取最后一次 lookup 的结果
            continue
        if found_cand and "." in line:
            parts = line.split(".")
            if len(parts) > 1:
                name = parts[1].split("(")[0].strip()
                candidates.append(name)
    return candidates

if __name__ == "__main__":
    # 确保已编译
    if not os.path.exists("./target/debug/rust-ime"):
        print("请先运行 cargo build")
        exit(1)

    # 清理历史
    if os.path.exists("data/user_data.db"):
        import shutil
        # sled 数据库通常是一个目录
        if os.path.isdir("data/user_data.db"):
            shutil.rmtree("data/user_data.db")
        else:
            os.remove("data/user_data.db")

    print("Step 1: 第一次搜索 'nihao'")
    cands1 = run_ime_cmd(["nihao"])
    if not cands1 or len(cands1) < 2:
        print(f"未能获取足够候选词: {cands1}")
        exit(1)
    
    first = cands1[0]
    second = cands1[1]
    print(f"1st: {first}, 2nd: {second}")

    print(f"\nStep 2: 模拟选择第二个词 '{second}'")
    # 在交互模式下输入 2 选择第二个词
    # 注意：我们的 --test 模式可能需要支持选择逻辑
    # 查阅 src/main.rs 里的测试逻辑
    
    # 运行第二次，包含选择逻辑
    # 第一次输入 nihao，然后输入 2 (假设数字键选择)
    # 然后再次输入 nihao 检查顺序
    cands2 = run_ime_cmd(["nihao", "2", "nihao"])
    
    if not cands2:
        print("第二次搜索未能获取候选词")
        exit(1)

    print(f"再次搜索 'nihao' 后的顺序:")
    print(f"1st: {cands2[0]}, 2nd: {cands2[1]}")

    if cands2[0] == second:
        print("\n✅ [成功] 调频生效！")
    else:
        print("\n❌ [失败] 调频未生效，第二个词仍然排在后面。")
        if second in cands2:
            print(f"'{second}' 现在排在第 {cands2.index(second) + 1} 位")
        else:
            print(f"'{second}' 不在候选词列表中")
