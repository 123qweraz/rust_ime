import json
import os

def extract_basic_tokens(input_file, output_file):
    if not os.path.exists(input_file):
        print(f"Error: {input_file} not found")
        return

    tokens = set()
    with open(input_file, 'r', encoding='utf-8') as f:
        data = json.load(f)
        for pinyin, entries in data.items():
            for entry in entries:
                if entry.get('category') == 'basic':
                    char = entry.get('char')
                    if char:
                        tokens.add(char)
    
    # Sort by length descending to help with greedy matching later
    sorted_tokens = sorted(list(tokens), key=len, reverse=True)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for token in sorted_tokens:
            f.write(token + '\n')
    
    print(f"Extracted {len(sorted_tokens)} basic tokens to {output_file}")

if __name__ == "__main__":
    extract_basic_tokens('dicts/words.json', 'dicts/basic_tokens.txt')

