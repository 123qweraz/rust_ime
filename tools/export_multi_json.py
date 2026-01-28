import json
import os

def export_multi_category_json(json_path, output_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Extracting multi-category words from {json_path} to {output_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    multi_data = {}
    total_count = 0
    
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            multi_entries = [entry for entry in entries if ',' in entry.get('category', '')]
            if multi_entries:
                multi_data[pinyin] = multi_entries
                total_count += len(multi_entries)

    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(multi_data, f, ensure_ascii=False, indent=2)

    print(f"Done. Saved {total_count} entries to {output_path}")

if __name__ == "__main__":
    export_multi_category_json("dicts/chinese/vocabulary/words.json", "dicts/chinese/vocabulary/multi_category_words.json")
