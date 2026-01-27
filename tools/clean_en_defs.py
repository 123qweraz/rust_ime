import json
import os
import re

def clean_en(en_str):
    if not isinstance(en_str, str):
        return en_str
    
    # Split by / or ; or , (common separators for multiple meanings)
    # We use a regex to handle these.
    parts = re.split(r'[/;,]', en_str)
    if parts:
        return parts[0].strip()
    return en_str

def process_file(file_path):
    print(f"Processing {file_path}...")
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        changed = False
        if isinstance(data, dict):
            for key in data:
                entries = data[key]
                if isinstance(entries, list):
                    for entry in entries:
                        if isinstance(entry, dict) and 'en' in entry:
                            old_en = entry['en']
                            new_en = clean_en(old_en)
                            if old_en != new_en:
                                entry['en'] = new_en
                                changed = True
        
        if changed:
            with open(file_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, ensure_ascii=False, indent=2)
            print(f"  Updated {file_path}")
            
    except Exception as e:
        print(f"  Error processing {file_path}: {e}")

def main():
    dicts_dir = 'dicts'
    for root, dirs, files in os.walk(dicts_dir):
        for file in files:
            if file.endswith('.json') and file != 'punctuation.json':
                process_file(os.path.join(root, file))

if __name__ == "__main__":
    main()
