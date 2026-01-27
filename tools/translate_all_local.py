#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import requests
import json
import os
from pypinyin import lazy_pinyin

# 配置
# 获取脚本所在目录的绝对路径，并定位到项目根目录
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.dirname(SCRIPT_DIR)
OLLAMA_URL = "http://localhost:11434/api/generate"
MODEL = "gemma3:latest" 
INPUT_DIR = os.path.join(BASE_DIR, "dicts/chinese/vocabulary/chuzhongcihui/untranslated")
OUTPUT_DIR = os.path.join(BASE_DIR, "dicts/chinese/vocabulary/chuzhongcihui/ai_translated")
BATCH_SIZE = 30 

def get_pinyin(word):
    return "".join(lazy_pinyin(word))

def translate_batch(words, subject):
    prompt = (
        f"You are a professional teacher. Translate these Chinese terms from the subject '{subject}' to English. "
        "Output ONLY 'Chinese: English', one per line. No extra text.\n\n"
        + "\n".join(words)
    )
    payload = {
        "model": MODEL, 
        "prompt": prompt, 
        "stream": False, 
        "options": {"temperature": 0.3}
    }
    try:
        response = requests.post(OLLAMA_URL, json=payload, timeout=180)
        if response.status_code == 200:
            text = response.json().get("response", "").strip()
            results = {}
            for line in text.split('\n'):
                line = line.strip()
                if not line: continue
                # 尝试分割 ':' 或 '：'
                for sep in [':', '：']:
                    if sep in line:
                        parts = line.split(sep, 1)
                        if len(parts) == 2:
                            results[parts[0].strip()] = parts[1].strip()
                        break
            return results
    except Exception as e:
        print(f"  API调用出错: {e}")
    return {}

def process_file(file_path, output_path, subject):
    if not os.path.exists(file_path): return
    with open(file_path, 'r', encoding='utf-8') as f:
        words = [line.strip() for line in f if line.strip()]

    final_data = {}
    if os.path.exists(output_path):
        with open(output_path, 'r', encoding='utf-8') as f:
            try:
                final_data = json.load(f)
            except:
                final_data = {}

    # 记录已翻译的词，避免重复
    translated_zh = {item['char'] for p in final_data for item in final_data[p]}
    remaining = [w for w in words if w not in translated_zh]
    
    if not remaining: 
        print(f"--- {subject} 已全部翻译完成。")
        return

    print(f">>> 开始翻译 {subject}: 剩余 {len(remaining)}/{len(words)}")

    for i in range(0, len(remaining), BATCH_SIZE):
        batch = remaining[i : i + BATCH_SIZE]
        translations = translate_batch(batch, subject)
        
        # 将翻译结果存入 final_data
        for zh in batch:
            en = translations.get(zh, "")
            # 如果AI没给翻译，保留占位符或跳过
            if not en: 
                continue 
            
            py = get_pinyin(zh)
            if py not in final_data: final_data[py] = []
            if not any(item['char'] == zh for item in final_data[py]):
                final_data[py].append({"char": zh, "en": en})
        
        # 每批次保存一次，防止断电或中断
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(final_data, f, ensure_ascii=False, indent=2)
        
        print(f"  已保存进度: {i + len(batch)}/{len(remaining)}")

def main():
    if not os.path.exists(OUTPUT_DIR): os.makedirs(OUTPUT_DIR)
    
    # 遍历 untranslated 目录下的所有文件
    for filename in os.listdir(INPUT_DIR):
        if not filename.endswith("_untranslated.txt"): continue
        
        # 识别学科名
        subject = filename.replace("_untranslated.txt", "")
        # 对应输出的 json 文件名
        json_filename = f"{subject}_ai.json"
        
        process_file(
            os.path.join(INPUT_DIR, filename), 
            os.path.join(OUTPUT_DIR, json_filename),
            subject
        )

if __name__ == "__main__":
    main()
