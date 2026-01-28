import json
import os
import csv

def export_multi_category_words(json_path, csv_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Extracting multi-category words from {json_path} to {csv_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    results = []
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                category = entry.get('category', '')
                if ',' in category:  # 包含逗号
                    results.append([
                        category,
                        pinyin,
                        entry.get('char', ''),
                        entry.get('en', '')
                    ])

    with open(csv_path, 'w', encoding='utf-8', newline='') as f:
        writer = csv.writer(f)
        # writer.writerow(["category", "pinyin", "char", "en"])
        writer.writerows(results)

    print(f"Done. Saved {len(results)} entries to {csv_path}")

if __name__ == "__main__":
    export_multi_category_words("dicts/chinese/vocabulary/words.json", "dicts/chinese/vocabulary/multi_category_words.csv")
