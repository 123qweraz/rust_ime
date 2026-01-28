import json
import os

def set_common_category(json_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Updating categories to 'common' for multi-disciplinary words in {json_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    updated_count = 0
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                category = entry.get('category', '')
                if ',' in category:
                    entry['category'] = 'common'
                    updated_count += 1

    with open(json_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)

    print(f"Done. Updated {updated_count} entries to 'common'.")

if __name__ == "__main__":
    # 更新主词库
    set_common_category("dicts/chinese/vocabulary/words.json")
    # 同时更新之前提取的跨学科词库
    set_common_category("dicts/chinese/vocabulary/multi_category_words.json")
