import os
import subprocess
import shutil
import time

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
    candidates = []
    
    # 逆序寻找最后一次显示的候选词块
    found_block = False
    for i in range(len(lines)-1, -1, -1):
        line = lines[i]
        if "候选词 (前 10 条):" in line:
            found_block = True
            # 向上收集直到块结束（即 reversed 时的向下）
            for j in range(i + 1, len(lines)):
                l = lines[j]
                if ". " in l and "(" in l:
                    candidates.append(l.strip())
                elif ">" in l or "预编辑:" in l:
                    break
            break
    
    return candidates

if __name__ == "__main__":
    print("--- 自学习 N-Gram (Bigram) 联想测试 ---")
    
    db_path = "data/user_data.db"
    if os.path.exists(db_path):
        shutil.rmtree(db_path)
        print("已清理旧数据库。")

    # Step 1: 训练模型 - 建立 "我" -> "是" 的联系
    # 输入 'wo' 按空格 (1) 上屏 '我'
    # 输入 'shi' 按 '2' (假设 '是' 在第 2 位) 上屏 '是'
    print("\n[Step 1] 正在训练: 建立 '我' -> '是' 的联系...")
    # 第一次运行，模拟选择
    # 注意: 我们需要知道 '是' 的确切位置。先查一下。
    init_shi = run_ime_cmd(["shi"])
    shi_idx = -1
    for i, c in enumerate(init_shi):
        if "是" in c:
            shi_idx = i + 1
            break
    
    if shi_idx == -1:
        print("未能在候选词中找到 '是'，测试中止。")
        exit(1)
    
    print(f"'是' 当前位于第 {shi_idx} 位。")
    
    # 模拟输入序列: wo -> Space (选1) -> shi -> (选shi_idx)
    run_ime_cmd(["wo", " ", "shi", str(shi_idx)])
    print("训练完成。")

    # Step 2: 验证联想 - 输入 'wo' 上屏后，输入 'shi' 看 '是' 是否置顶
    print("\n[Step 2] 正在验证: 再次输入 'wo' (上屏) 后输入 'shi'...")
    # 序列: wo -> Space -> shi
    res = run_ime_cmd(["wo", " ", "shi"])
    
    if not res:
        print("未能获取候选词列表。")
        exit(1)

    first_cand = res[0]
    print(f"当前首选词: {first_cand}")

    if "是" in first_cand and "(Context)" in first_cand:
        print("\n✅ [成功] 自学习 N-Gram 生效！'是' 已根据上下文成功置顶并标记。")
    else:
        print("\n❌ [失败] 联想未生效。")
        print(f"预期首选包含 '是' 和 '(Context)'，实际为: '{first_cand}'")
