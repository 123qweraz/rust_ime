import json
import os

def clean_pseudo_words(json_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    print(f"Cleaning pseudo-words (OCR errors) in {json_path}...")
    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    # 定义一些高频误识别字
    # 这些字如果成对出现，或者在特定语境（如数学）下出现，通常是 A, B, C, D 的误读
    garbage_chars = '犃犅犆犇狀狀狀狀狀状' # 包含之前清理过的和一些变体
    suspicious_starts = ['地', '帝', '状', '点', '图', '线']
    
    # 明确要删除的垃圾词示例
    specific_garbage = ['帝帝', '地滴', '地地', '状状', '状个', '状天', '状是', '状状状']

    cleaned_data = {}
    removed_count = 0
    removed_list = []

    for pinyin, entries in data.items():
        if not isinstance(entries, list):
            cleaned_data[pinyin] = entries
            continue
            
        new_entries = []
        for entry in entries:
            char = entry.get('char', '')
            category = entry.get('category', '')
            
            should_remove = False
            
            # 1. 在特定列表中
            if char in specific_garbage:
                should_remove = True
            
            # 2. 只有两个字且两个字相同，且属于非 basic 类别（尤其是数学/共有）
            elif len(char) == 2 and char[0] == char[1] and category in ['mathematics', 'common', 'chinese']:
                # 排除一些正常的叠词
                if char not in ['天天', '人人', '大大', '看看', '听听', '好好']:
                    # 检查是否包含可疑字
                    if any(c in char for c in '帝状个地'):
                        should_remove = True
            
            # 3. 包含明显的残留垃圾字符
            elif any(c in char for c in '犃犅犆犇狀'):
                should_remove = True

            if should_remove:
                removed_count += 1
                if len(removed_list) < 20:
                    removed_list.append(char)
                continue
            
            new_entries.append(entry)
        
        if new_entries:
            cleaned_data[pinyin] = new_entries

    with open(json_path, 'w', encoding='utf-8') as f:
        json.dump(cleaned_data, f, ensure_ascii=False, indent=2)

    print(f"Done. Removed {removed_count} pseudo-words.")
    if removed_list:
        print(f"Sample removed: {', '.join(removed_list)}")

if __name__ == "__main__":
    json_files = [
        "dicts/chinese/vocabulary/words.json",
        "dicts/chinese/vocabulary/multi_category_words.json",
        "dicts/archive/vocabulary/chuzhongcihui/Mathematics.json"
    ]
    for f in json_files:
        clean_pseudo_words(f)
