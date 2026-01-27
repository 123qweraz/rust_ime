#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import requests
import json
import os
from pypinyin import lazy_pinyin

# 配置
OLLAMA_URL = "http://localhost:11434/api/generate"
MODEL = "gemma3:latest"
INPUT_DIR = "all_vocabulary"
OUTPUT_DIR = "translated_vocabulary"
BATCH_SIZE = 30 

# 学科名称翻译映射
SUBJECT_MAP = {
    "语文": "Chinese",
    "数学": "Mathematics",
    "英语": "English",
    "物理": "Physics",
    "化学": "Chemistry",
    "生物学": "Biology",
    "地理": "Geography",
    "历史": "History",
    "体育与健康": "PE_and_Health",
    "外语": "Foreign_Languages",
    "俄语": "Russian",
    "美术": "Arts",
    "音乐": "Music",
    "道德与法治": "Politics",
    "其他": "Others"
}

def get_pinyin(word):
    return "".join(lazy_pinyin(word))

def translate_batch(words):
    prompt = f"Translate these Chinese terms to English for a textbook glossary. Output ONLY 'Chinese: English', one per line.\n\n" + "\n".join(words)
    payload = {"model": MODEL, "prompt": prompt, "stream": False, "options": {"temperature": 0.3}}
    try:
        response = requests.post(OLLAMA_URL, json=payload, timeout=120)
        if response.status_code == 200:
            text = response.json().get("response", "").strip()
            results = {}
            for line in text.split('\n'):
                if ':' in line:
                    parts = line.split(':', 1)
                    results[parts[0].strip()] = parts[1].strip()
            return results
    except:
        pass
    return {}

def process_file(file_path, output_path):
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

    translated_zh = {item['char'] for p in final_data for item in final_data[p]}
    remaining = [w for w in words if w not in translated_zh]
    
    if not remaining: 
        print(f"文件 {os.path.basename(file_path)} 已完成。")
        return

    print(f"正在翻译 {os.path.basename(file_path)}: 剩余 {len(remaining)}/{len(words)}")

    for i in range(0, len(remaining), BATCH_SIZE):
        batch = remaining[i : i + BATCH_SIZE]
        print(f"  进度: {i}/{len(remaining)}...")
        translations = translate_batch(batch)
        
        for zh in batch:
            en = translations.get(zh, "")
            py = get_pinyin(zh)
            if py not in final_data: final_data[py] = []
            if not any(item['char'] == zh for item in final_data[py]):
                final_data[py].append({"char": zh, "en": en})
        
        with open(output_path, 'w', encoding='utf-8') as f:
            json.dump(final_data, f, ensure_ascii=False, indent=2)

def main():
    if not os.path.exists(OUTPUT_DIR): os.makedirs(OUTPUT_DIR)
    if not os.path.exists(INPUT_DIR):
        print(f"错误: 找不到目录 {INPUT_DIR}")
        return
        
    for filename in os.listdir(INPUT_DIR):
        if not filename.endswith("_词汇表.txt"): continue
        zh_subject = filename.replace("_词汇表.txt", "")
        en_subject = SUBJECT_MAP.get(zh_subject, zh_subject)
        process_file(os.path.join(INPUT_DIR, filename), os.path.join(OUTPUT_DIR, f"{en_subject}.json"))

if __name__ == "__main__":
    main()
