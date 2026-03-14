import os
import re
import json
from pypinyin import pinyin, Style

def get_stroke_mapping():
    # 5x5 矩阵映射
    mapping = {
        '11': 'g', '12': 'f', '13': 'd', '14': 's', '15': 'a',
        '21': 'h', '22': 'j', '23': 'k', '24': 'l', '25': 'm',
        '31': 't', '32': 'r', '33': 'e', '34': 'w', '35': 'q',
        '41': 'y', '42': 'u', '43': 'i', '44': 'o', '45': 'p',
        '51': 'n', '52': 'b', '53': 'v', '54': 'c', '55': 'x'
    }
    # 单笔映射 (补齐时使用)
    single_mapping = {'1': 'g', '2': 'h', '3': 't', '4': 'y', '5': 'n'}
    return mapping, single_mapping

def encode_pair(pair, mapping, single_mapping):
    if len(pair) == 2:
        return mapping.get(pair, '')
    elif len(pair) == 1:
        return single_mapping.get(pair, '')
    return ''

def get_pinyin_first_letter(char):
    res = pinyin(char, style=Style.FIRST_LETTER)
    if res and res[0] and res[0][0]:
        return res[0][0].lower()
    return 'z' # Fallback

def parse_js_data(file_path):
    char_to_strokes = {}
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
            # 匹配 ["汉字","笔画"]
            matches = re.findall(r'\[\s*["\'](.*?)["\']\s*,\s*["\'](.*?)["\']\s*\]', content)
            for char, strokes in matches:
                clean_strokes = "".join(filter(str.isdigit, strokes))
                if char and clean_strokes:
                    char_to_strokes[char] = clean_strokes
    except Exception as e:
        print(f"Error: {e}")
    return char_to_strokes

def main():
    mapping, single_mapping = get_stroke_mapping()
    js_path = 'referdict/data_v2.js'
    
    print(f"Reading {js_path}...")
    char_to_strokes = parse_js_data(js_path)
    
    encoded_dict = {}
    syllables = set()
    
    print("Processing characters...")
    for char, strokes in char_to_strokes.items():
        # 1. 前4笔 -> 前2个字母
        first_4 = strokes[:4]
        part1 = ""
        # 前两笔 -> 第1字母
        part1 += encode_pair(first_4[:2], mapping, single_mapping)
        # 第3-4笔 -> 第2字母
        if len(first_4) > 2:
            part1 += encode_pair(first_4[2:4], mapping, single_mapping)
        
        # 补齐逻辑：如果不足4笔，part1 长度可能小于 2
        while len(part1) < 2:
            part1 += "z" # 或者其他占位符，这里暂用 z
            
        # 2. 最后2笔 -> 第3个字母
        last_2 = strokes[-2:] if len(strokes) >= 2 else strokes
        part2 = encode_pair(last_2, mapping, single_mapping)
        if not part2:
            part2 = "z"
            
        # 3. 拼音首字母 -> 第4个字母
        py = get_pinyin_first_letter(char)
        
        full_code = (part1 + part2 + py).lower()
        
        if full_code not in encoded_dict:
            encoded_dict[full_code] = []
        
        # 模拟原始字典结构
        encoded_dict[full_code].append({
            "char": char,
            "weight": 100, # 默认权重，之后可以根据词频修正
            "tone": "",    # 占位
            "trad": char,
            "en": "",
            "category": "common"
        })
        syllables.add(full_code)

    output_dir = 'dicts/stroke/words'
    os.makedirs(output_dir, exist_ok=True)
    
    char_json_path = os.path.join(output_dir, 'stroke_char.json')
    with open(char_json_path, 'w', encoding='utf-8') as f:
        json.dump(encoded_dict, f, ensure_ascii=False, indent=2)
        
    syllables_path = 'dicts/stroke/syllables.txt'
    with open(syllables_path, 'w', encoding='utf-8') as f:
        for s in sorted(list(syllables)):
            f.write(f"{s}\n")
            
    print(f"Done! Created {char_json_path} and {syllables_path}")
    print(f"Total codes: {len(encoded_dict)}")

if __name__ == "__main__":
    main()
