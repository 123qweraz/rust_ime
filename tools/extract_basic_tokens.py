import json
import os
import re

def extract_tokens_from_final_dicts(words_file, chars_file, output_file):
    tokens = set()
    
    # 匹配 JSON 中的 "char": "汉字"
    char_pattern = re.compile(r'"char":\s*"([^"]+)"')

    # 1. 从 chars.json 提取单字 (基石)
    if os.path.exists(chars_file):
        with open(chars_file, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                match = char_pattern.search(line)
                if match:
                    char = match.group(1)
                    if len(char) == 1:
                        tokens.add(char)
        print(f"Collected {len(tokens)} single characters from {chars_file}.")

    # 2. 从 basic_words.json 提取词组 (仅 basic 类别)
    if os.path.exists(words_file):
        with open(words_file, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                match = char_pattern.search(line)
                if match:
                    word = match.group(1)
                    # 这里的词已经全部是 basic 了，我们提取 2-4 字的
                    if 2 <= len(word) <= 4:
                        tokens.add(word)
        print(f"Total unique tokens after adding basic phrases from {words_file}: {len(tokens)}")

    # 3. 排序并保存
    sorted_tokens = sorted(list(tokens), key=len, reverse=True)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for token in sorted_tokens:
            f.write(token + '\n')
    
    print(f"Final token list saved to {output_file}")

if __name__ == "__main__":
    # 使用你刚才生成的 basic_words.json
    extract_tokens_from_final_dicts('dicts/basic_words.json', 'dicts/chars.json', 'dicts/basic_tokens.txt')