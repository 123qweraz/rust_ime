import json
import os
import re

def clean_en_text(text):
    if not isinstance(text, str):
        return text
    
    # 1. 移除词性标注，如 (n), (v), (adj), (adv), (prep), (pron), (conj), (intj)
    # 不分大小写，移除带括号的部分
    pos_pattern = r'\((n|v|adj|adv|prep|pron|conj|intj|v-n|v-adj|num|classifier)\)\s*'
    text = re.sub(pos_pattern, '', text, flags=re.IGNORECASE)
    
    # 2. 移除括号内的所有注释内容，如 (a place name), (likely meant to be...)
    # 匹配圆括号和方括号
    text = re.sub(r'\(.*?\)', '', text)
    text = re.sub(r'\[.*?\]', '', text)
    
    # 3. 清理多余空格
    text = re.sub(r'\s+', ' ', text).strip()
    
    return text

def clean_file(file_path):
    if not os.path.exists(file_path):
        print(f"File {file_path} not found.")
        return

    print(f"Cleaning English translations in {file_path}...")
    with open(file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    cleaned_count = 0
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                old_en = entry.get('en', '')
                if old_en:
                    new_en = clean_en_text(old_en)
                    if new_en != old_en:
                        entry['en'] = new_en
                        cleaned_count += 1
        elif isinstance(entries, str):
            # 处理非列表情况（如果有）
            pass

    with open(file_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

    print(f"Done. Cleaned {cleaned_count} translation entries.")

if __name__ == "__main__":
    json_files = [
        "dicts/chinese/vocabulary/words.json",
        "dicts/chinese/vocabulary/multi_category_words.json",
        "dicts/archive/vocabulary/chuzhongcihui/Mathematics.json"
    ]
    for f in json_files:
        clean_file(f)
