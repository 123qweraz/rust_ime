import re
import os

def extract_tokens_via_regex(words_file, chars_file, output_file):
    tokens = set()
    
    # 匹配 JSON 中的 "char": "汉字"
    char_pattern = re.compile(r'"char":\s*"([^"]+)"')

    # 1. 处理单字库
    if os.path.exists(chars_file):
        with open(chars_file, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                match = char_pattern.search(line)
                if match:
                    char = match.group(1)
                    if len(char) == 1:
                        tokens.add(char)
        print(f"Collected {len(tokens)} single characters.")

    # 2. 处理词组库 (仅提取长度 2-4 的词)
    if os.path.exists(words_file):
        # 简单起见，我们提取所有匹配到的 "char"
        # 然后过滤长度
        with open(words_file, 'r', encoding='utf-8', errors='ignore') as f:
            for line in f:
                match = char_pattern.search(line)
                if match:
                    word = match.group(1)
                    if 2 <= len(word) <= 4:
                        tokens.add(word)
        print(f"Total unique tokens after adding phrases: {len(tokens)}")

    # 3. 排序并保存
    # 按长度降序，这是贪婪匹配算法的关键
    sorted_tokens = sorted(list(tokens), key=len, reverse=True)
    
    with open(output_file, 'w', encoding='utf-8') as f:
        for token in sorted_tokens:
            f.write(token + '\n')
    
    print(f"Final token list saved to {output_file}")

if __name__ == "__main__":
    extract_tokens_via_regex('dicts/words.json', 'dicts/chars.json', 'dicts/basic_tokens.txt')
