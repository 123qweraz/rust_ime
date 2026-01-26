import json
import os

INPUT_FILE = "dicts/chinese/character/level-1_char_en.json"
OUTPUT_FILE = "dicts/chinese/character/level-1_en_char.txt"
OUTPUT_JSON = "dicts/chinese/character/level-1_en_char.json"

def main():
    if not os.path.exists(INPUT_FILE):
        print(f"Error: {INPUT_FILE} not found.")
        return

    with open(INPUT_FILE, 'r', encoding='utf-8') as f:
        data = json.load(f)

    en_char_pairs = set()
    en_to_chars = {} # For JSON: { "en": ["char1", "char2"] }

    # data format: {"pinyin": [{"char": "...", "en": "..."}, ...], ...}
    for entries in data.values():
        for entry in entries:
            char = entry.get('char')
            en = entry.get('en')
            if char and en:
                en_char_pairs.add(f"{en},{char}")
                
                en_lower = en.lower()
                if en_lower not in en_to_chars:
                    en_to_chars[en_lower] = []
                if char not in en_to_chars[en_lower]:
                    en_to_chars[en_lower].append(char)

    # Text output
    sorted_pairs = sorted(list(en_char_pairs))
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        for line in sorted_pairs:
            f.write(line + "\n")

    # JSON output
    # Format it to match expected CharEnEntry or similar
    # Actually, let's make it easy to load. 
    # Current load_char_en_map expects: { "some_key": [ {"char": "...", "en": "..."}, ... ] }
    json_data = {
        "entries": []
    }
    for en, chars in en_to_chars.items():
        for char in chars:
            json_data["entries"].append({"char": char, "en": en})

    with open(OUTPUT_JSON, 'w', encoding='utf-8') as f:
        json.dump(json_data, f, ensure_ascii=False, indent=2)

    print(f"Files generated: {OUTPUT_FILE}, {OUTPUT_JSON}")
    print(f"Total pairs: {len(sorted_pairs)}")
    print(f"Total unique English words: {len(en_to_chars)}")

if __name__ == "__main__":
    main()
