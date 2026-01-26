import json
import os

INPUT_FILE = "dicts/chinese/character/level-1_char_en.json"
OUTPUT_FILE = "dicts/chinese/character/level-1_en_char.txt"

def main():
    if not os.path.exists(INPUT_FILE):
        print(f"Error: {INPUT_FILE} not found.")
        return

    with open(INPUT_FILE, 'r', encoding='utf-8') as f:
        data = json.load(f)

    en_char_pairs = []

    # data format: {"pinyin": [{"char": "...", "en": "..."}, ...], ...}
    for entries in data.values():
        for entry in entries:
            char = entry.get('char')
            en = entry.get('en')
            if char and en:
                # Some 'en' might have multiple words or punctuation, 
                # but we'll follow the requested 'en,char' format.
                en_char_pairs.append(f"{en},{char}")

    # Sort alphabetically by English word for better usability
    en_char_pairs.sort()

    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        for line in en_char_pairs:
            f.write(line + "\n")

    print(f"File generated: {OUTPUT_FILE}")
    print(f"Total entries: {len(en_char_pairs)}")

if __name__ == "__main__":
    main()
