import json
import os
from collections import defaultdict

def process_dictionaries(dir_path):
    # Map (pinyin, char) -> list of (en, filename)
    word_map = defaultdict(list)
    
    files = [f for f in os.listdir(dir_path) if f.endswith('.json') and f not in ['common_vocabulary.json', 'duplicates.json']]
    
    # 1. Collect all occurrences
    for filename in files:
        file_path = os.path.join(dir_path, filename)
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
            for pinyin, entries in data.items():
                for entry in entries:
                    word_map[(pinyin, entry['char'])].append({
                        'en': entry.get('en', ''),
                        'file': filename
                    })

    # 2. Identify common words (>= 5 files)
    common_data = defaultdict(list)
    common_keys = set()
    
    for (pinyin, char), occurrences in word_map.items():
        if len(occurrences) >= 5:
            # Keep one entry (the first one)
            common_data[pinyin].append({
                'char': char,
                'en': occurrences[0]['en']
            })
            common_keys.add((pinyin, char))

    # 3. Rewrite original files removing common words
    for filename in files:
        file_path = os.path.join(dir_path, filename)
        with open(file_path, 'r', encoding='utf-8') as f:
            data = json.load(f)
        
        new_data = {}
        for pinyin, entries in data.items():
            filtered_entries = [e for e in entries if (pinyin, e['char']) not in common_keys]
            if filtered_entries:
                new_data[pinyin] = filtered_entries
        
        with open(file_path, 'w', encoding='utf-8') as f:
            json.dump(new_data, f, ensure_ascii=False, indent=2)

    # 4. Write common words to new file
    output_path = os.path.join(dir_path, 'common_vocabulary.json')
    with open(output_path, 'w', encoding='utf-8') as f:
        json.dump(common_data, f, ensure_ascii=False, indent=2)
        
    return len(common_keys)

if __name__ == "__main__":
    count = process_dictionaries('dicts/chinese/vocabulary/chuzhongcihui/')
    print(f"Processed {count} common words moved to common_vocabulary.json")
