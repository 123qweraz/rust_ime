import re
import os
import json

def extract_basic_words_robust(input_file, output_file):
    if not os.path.exists(input_file):
        print(f"Error: {input_file} not found")
        return

    basic_data = {}
    print(f"Streaming {input_file} for basic words...")
    
    with open(input_file, 'r', encoding='utf-8', errors='ignore') as f:
        content = f.read()
        blocks = re.split(r'^\s*"([^"]+)":\s*\[', content, flags=re.MULTILINE)
        
        for i in range(1, len(blocks), 2):
            py = blocks[i]
            block_content = blocks[i+1]
            entry_matches = re.findall(r'\{[^{}]+\}', block_content)
            
            filtered = []
            for entry_str in entry_matches:
                if '"category": "basic"' in entry_str:
                    char_match = re.search(r'"char":\s*"([^"]+)"', entry_str)
                    en_match = re.search(r'"en":\s*"([^"]+)"', entry_str)
                    
                    if char_match:
                        entry_obj = {"char": char_match.group(1), "category": "basic"}
                        if en_match:
                            entry_obj["en"] = en_match.group(1)
                        filtered.append(entry_obj)
            
            if filtered:
                basic_data[py] = filtered

    print(f"Saving {len(basic_data)} basic pinyin groups to {output_file}...")
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(basic_data, f, ensure_ascii=False, indent=2)
    
    print("Success!")

if __name__ == "__main__":
    extract_basic_words_robust('dicts/words.json', 'dicts/basic_words.json')
