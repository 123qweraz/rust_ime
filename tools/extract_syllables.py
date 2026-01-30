import json
import os

def extract_syllables():
    chars_file = 'dicts/chinese/chars.json'
    output_file = 'dicts/chinese/syllables.txt'
    
    if not os.path.exists(chars_file):
        print(f"Error: {chars_file} not found")
        return

    with open(chars_file, 'r', encoding='utf-8') as f:
        data = json.load(f)
    
    # 提取所有键（即拼音音节）并排序
    syllables = sorted(list(data.keys()))
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for s in syllables:
            if s.strip():
                f.write(s.strip() + '\n')
    
    print(f"Successfully extracted {len(syllables)} syllables to {output_file}")

if __name__ == "__main__":
    extract_syllables()
