import json
import os

def export_for_review(json_path, output_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Exporting words for review from {json_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    unique_pairs = set()
    
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                word = entry.get('char', '')
                translation = entry.get('en', '')
                if word:
                    unique_pairs.add((word, translation))

    sorted_pairs = sorted(list(unique_pairs))

    with open(output_path, 'w', encoding='utf-8') as f:
        for word, en in sorted_pairs:
            f.write(f"{word} | {en}\n")

    print(f"Done. Saved {len(sorted_pairs)} unique items to {output_path}")

if __name__ == "__main__":
    export_for_review("dicts/chinese/vocabulary/words.json", "dicts/chinese/vocabulary/review_list.txt")