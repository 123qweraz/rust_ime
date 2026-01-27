import json
import os

def capitalize_en_in_file(file_path):
    if not os.path.exists(file_path):
        print(f"Skipping {file_path}, file not found.")
        return

    print(f"Processing {file_path}...")
    with open(file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    count = 0
    for pinyin in data:
        for entry in data[pinyin]:
            en = entry.get('en', '')
            if en and isinstance(en, str):
                # 将首字母转为大写
                new_en = en[0].upper() + en[1:]
                if new_en != en:
                    entry['en'] = new_en
                    count += 1

    with open(file_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
    
    print(f"Done. Updated {count} entries in {file_path}")

if __name__ == "__main__":
    capitalize_en_in_file("dicts/chinese/character/chars.json")
    capitalize_en_in_file("dicts/chinese/vocabulary/words.json")
