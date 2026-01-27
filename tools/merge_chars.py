import json
import os
import glob

def capitalize_first(s):
    if not s or not isinstance(s, str):
        return s
    return s[0].upper() + s[1:]

def merge_reorganized():
    # --- 1. 处理单字 (Characters) ---
    char_dir = "dicts/archive/character"
    char_output = "dicts/chinese/character/chars.json"
    char_files = {
        "level-1": "level-1_char_en.json",
        "level-2": "level-2_char_en.json",
        "level-3": "level-3_char_en.json"
    }
    
    char_data = {}
    for category, filename in char_files.items():
        path = os.path.join(char_dir, filename)
        if os.path.exists(path):
            with open(path, 'r', encoding='utf-8') as f:
                data = json.load(f)
                for pinyin, entries in data.items():
                    if pinyin not in char_data: char_data[pinyin] = []
                    for entry in entries:
                        entry['category'] = category
                        # 英文首字母大写
                        entry['en'] = capitalize_first(entry.get('en', ''))
                        char_data[pinyin].append(entry)
    
    os.makedirs(os.path.dirname(char_output), exist_ok=True)
    with open(char_output, 'w', encoding='utf-8') as f:
        json.dump(char_data, f, ensure_ascii=False, indent=2)
    print(f"Created {char_output} (English capitalized)")

    # --- 2. 处理词组 (Words) ---
    vocab_output = "dicts/chinese/vocabulary/words.json"
    cizu_path = "dicts/archive/vocabulary/dict_cizu.json"
    academic_dir = "dicts/archive/vocabulary/chuzhongcihui"
    
    word_data = {}

    def add_to_word_data(pinyin, entry, category):
        if pinyin not in word_data:
            word_data[pinyin] = []
        
        # 英文首字母大写
        en_def = capitalize_first(entry.get('en', ''))
        
        existing_entry = next((e for e in word_data[pinyin] if e['char'] == entry['char']), None)
        
        if existing_entry:
            if existing_entry['category'] == "basic":
                return
            if category == "basic":
                existing_entry['category'] = "basic"
            elif category not in existing_entry['category']:
                existing_entry['category'] += f", {category}"
            
            # 如果原条目没英文，补上大写后的英文
            if not existing_entry.get('en') and en_def:
                existing_entry['en'] = en_def
        else:
            word_data[pinyin].append({
                "char": entry['char'],
                "en": en_def,
                "category": category
            })

    # 加载基础词库
    if os.path.exists(cizu_path):
        print(f"Processing basic vocabulary...")
        with open(cizu_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            for pinyin, entries in data.items():
                for entry in entries:
                    add_to_word_data(pinyin, entry, 'basic')
    
    # 加载学科词库
    academic_files = glob.glob(os.path.join(academic_dir, "*.json"))
    for path in academic_files:
        subject = os.path.basename(path).replace(".json", "").lower()
        if subject == "common_vocabulary": subject = "general"
        
        print(f"Processing subject: {subject}...")
        with open(path, 'r', encoding='utf-8') as f:
            try:
                data = json.load(f)
                for pinyin, entries in data.items():
                    for entry in entries:
                        add_to_word_data(pinyin, entry, subject)
            except Exception as e:
                print(f"Error loading {path}: {e}")

    with open(vocab_output, 'w', encoding='utf-8') as f:
        json.dump(word_data, f, ensure_ascii=False, indent=2)
    print(f"Created {vocab_output} (English capitalized)")

if __name__ == "__main__":
    merge_reorganized()