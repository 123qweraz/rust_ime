import json
import os
import re

def clean_json_file(file_path, garbled_chars):
    if not os.path.exists(file_path):
        print(f"File {file_path} not found.")
        return

    print(f"Cleaning {file_path}...")
    with open(file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    cleaned_data = {}
    pattern = f"[{''.join(garbled_chars)}]"
    
    removed_count = 0
    for key, entries in data.items():
        if isinstance(entries, list):
            new_entries = []
            for entry in entries:
                # Check 'char' and 'en' fields
                char_val = entry.get('char', '')
                en_val = entry.get('en', '')
                if re.search(pattern, char_val) or re.search(pattern, str(en_val)):
                    removed_count += 1
                    continue
                new_entries.append(entry)
            
            if new_entries:
                cleaned_data[key] = new_entries
        else:
            # Handle cases where value is not a list if any
            if not re.search(pattern, str(entries)):
                cleaned_data[key] = entries
            else:
                removed_count += 1

    with open(file_path, 'w', encoding='utf-8') as f:
        json.dump(cleaned_data, f, ensure_ascii=False, indent=2)
    
    print(f"Removed {removed_count} entries from {file_path}.")

def clean_txt_file(file_path, garbled_chars):
    if not os.path.exists(file_path):
        print(f"File {file_path} not found.")
        return

    print(f"Cleaning {file_path}...")
    pattern = f"[{''.join(garbled_chars)}]"
    
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    new_lines = []
    removed_count = 0
    for line in lines:
        if re.search(pattern, line):
            removed_count += 1
            continue
        new_lines.append(line)

    with open(file_path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)
    
    print(f"Removed {removed_count} lines from {file_path}.")

if __name__ == "__main__":
    garbled = ["犛", "狀", "犃", "犆", "犅", "犇"]
    
    # JSON files to clean
    json_files = [
        "dicts/chinese/vocabulary/words.json",
        "dicts/archive/vocabulary/chuzhongcihui/Mathematics.json",
        "dicts/chinese/character/chars.json",
        "dicts/chinese/other/dict_cedict.json"
    ]
    
    for f in json_files:
        clean_json_file(f, garbled)

    # TXT files to clean
    txt_files = [
        "dicts/chinese/vocabulary/chuzhongcihui/new_txts/new_数学_词汇表.txt",
        "dicts/chinese/vocabulary/chuzhongcihui/all_vocabulary/数学_词汇表.txt"
    ]
    
    for f in txt_files:
        clean_txt_file(f, garbled)
