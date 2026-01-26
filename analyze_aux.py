import json
import os

INPUT_FILE = "dicts/chinese/character/level-1_char_en.json"
OUTPUT_FILE = "dicts/chinese/character/level-1_uniqueness_report.txt"
HARD_WORDS_FILE = "dicts/chinese/character/level-1_hard_words.txt"

def get_shortest_unique_prefix_len(target_word, other_words):
    target_len = len(target_word)
    
    # 尝试长度从 1 到 target_len
    for length in range(1, target_len + 1):
        prefix = target_word[:length]
        is_unique = True
        
        for other in other_words:
            # 如果其他单词以这个前缀开头，说明不唯一
            # 注意：如果其他单词比当前前缀短，startswith 会返回 False，这是对的
            # 例如 target="apple", prefix="app", other="app" -> 冲突
            # target="apple", prefix="app", other="ap" -> 不冲突 (因为 typed "app" 已经过滤掉 "ap")
            # 但是输入法逻辑通常是过滤。如果输入 "app"，"application" (匹配) 和 "apple" (匹配) 和 "ape" (不匹配)。
            # 我们要找的是：在这个前缀下，只剩下 target 这一个词（或者 target 是这组匹配中唯一的）。
            
            # 严格唯一性：其他单词不能以这个 prefix 开头
            if other.startswith(prefix):
                is_unique = False
                break
        
        if is_unique:
            return length
            
    # 如果全拼完了还和其他词有共同前缀（或者是其他词的前缀），
    # 比如 target="car", others=["cart"]。
    # 输入 "car"，"cart" 也会留下来。
    # 此时如果 target "car" 本身已经是完整词，我们认为它是唯一的（因为它最短）。
    # 但如果 target="cart", others=["car"]。输入 "c", "ca", "car" 都无法排除 "car"。
    # 必须输入 "cart" 才能排除 "car"。
    return target_len

def format_output(pinyin, en_word, unique_len, char):
    if not en_word:
        return f"{pinyin}{{NO_EN}}, {char}"

    # 整个单词转为小写处理逻辑
    word = en_word.lower()
    
    # 提取唯一前缀和剩余部分
    unique_part = word[:unique_len]
    suffix_part = word[unique_len:]
    
    # 第一个字母大写，其余全部小写
    first_letter = unique_part[0].upper()
    rest_of_unique = unique_part[1:].lower()
    rest_of_word = suffix_part.lower()
    
    if rest_of_word:
        formatted_code = f"{first_letter}{rest_of_unique}_{rest_of_word}"
    else:
        # 如果已经到了单词末尾才唯一，就不加下划线了
        formatted_code = f"{first_letter}{rest_of_unique}"
        
    return f"{pinyin}{formatted_code}, {char}"

def main():
    if not os.path.exists(INPUT_FILE):
        print(f"Error: {INPUT_FILE} not found.")
        return

    with open(INPUT_FILE, 'r', encoding='utf-8') as f:
        data = json.load(f)

    results = []

    # 遍历字典
    # data format: {"li": [{"char": "里", "en": "inside"}, ...], ...}
    for pinyin, entries in data.items():
        # 过滤掉没有 'en' 字段或 'en' 为空的条目，避免干扰计算
        valid_entries = [e for e in entries if e.get('en')]
        
        for entry in valid_entries:
            char = entry['char']
            en_word = entry['en'].lower() # 统一小写比较
            
            # 获取同音字的其他英文码
            other_words = []
            for other_entry in valid_entries:
                if other_entry['char'] != char:
                    other_words.append(other_entry['en'].lower())
            
            unique_len = get_shortest_unique_prefix_len(en_word, other_words)
            
            line = format_output(pinyin, entry['en'], unique_len, char)
            results.append(line)

    # 排序输出：按拼音字母序，但要保证同拼音的在一起
    results.sort()

    hard_entries = [] # 存储辅码超过2位的条目

    with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
        last_pinyin = ""
        for line in results:
            # 格式: pinyinUnique_suffix, char
            # 解析 line 找出 unique length
            # 1. 找到第一个大写字母的位置 (unique start)
            unique_start_idx = -1
            for i, c in enumerate(line):
                if c.isupper():
                    unique_start_idx = i
                    break
            
            if unique_start_idx != -1:
                # 2. 找到下划线位置 (unique end)
                underscore_idx = line.find('_', unique_start_idx)
                if underscore_idx != -1:
                    u_len = underscore_idx - unique_start_idx
                else:
                    # 如果没有下划线，说明一直到逗号前都是 unique
                    comma_idx = line.find(',', unique_start_idx)
                    u_len = comma_idx - unique_start_idx
                
                if u_len > 2:
                    hard_entries.append(line)

            # 提取当前行的拼音部分
            current_pinyin = line[:unique_start_idx] if unique_start_idx != -1 else ""
            
            if last_pinyin and current_pinyin != last_pinyin:
                f.write("\n")
            
            f.write(line + "\n")
            last_pinyin = current_pinyin

    print(f"Report generated: {OUTPUT_FILE}")
    print(f"Total entries: {len(results)}")
    
    print(f"\nStats: Entries requiring > 2 keystrokes: {len(hard_entries)}")
    if hard_entries:
        with open(HARD_WORDS_FILE, "w", encoding="utf-8") as f:
            for line in hard_entries:
                f.write(line + "\n")
        print(f"Detailed list saved to: {HARD_WORDS_FILE}")
        # Print first few examples
        print("Examples:")
        for line in hard_entries[:5]:
            print("  " + line)

if __name__ == "__main__":
    main()
