import os
import subprocess

def test_pinyin(pinyin):
    cmd = f"./target/debug/rust-ime --test {pinyin}"
    res = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    print(f"--- 拼音 '{pinyin}' 的结果 ---")
    print(res.stdout)

if __name__ == "__main__":
    test_pinyin("sm")
    test_pinyin("zm")
