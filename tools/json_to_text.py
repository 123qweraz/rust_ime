import json
import os
import csv

def json_to_csv(json_path, csv_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Converting {json_path} to {csv_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    with open(csv_path, 'w', encoding='utf-8', newline='') as f:
        writer = csv.writer(f)
        # 写入表头 (可选，如果不需要可以注释掉)
        # writer.writerow(["category", "pinyin", "char", "en"])
        
        for pinyin, entries in data.items():
            if isinstance(entries, list):
                for entry in entries:
                    char = entry.get('char', '')
                    en = entry.get('en', '')
                    category = entry.get('category', '')
                    # 顺序: 分类, 拼音, 词组, 翻译
                    writer.writerow([category, pinyin, char, en])
            else:
                writer.writerow(["", pinyin, entries, ""])

    print(f"Done. Created {csv_path}")

if __name__ == "__main__":
    json_to_csv("dicts/chinese/vocabulary/words.json", "dicts/chinese/vocabulary/words.csv")
    json_to_csv("dicts/archive/vocabulary/chuzhongcihui/Mathematics.json", "dicts/archive/vocabulary/chuzhongcihui/Mathematics.csv")