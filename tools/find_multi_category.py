import json
import os

def find_words_with_multiple_categories(json_path):
    if not os.path.exists(json_path):
        print(f"File {json_path} not found.")
        return

    with open(json_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    results = []
    for pinyin, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                category = entry.get('category', '')
                if ',' in category:  # 包含逗号说明有多个学科
                    results.append({
                        "pinyin": pinyin,
                        "char": entry.get('char', ''),
                        "category": category,
                        "en": entry.get('en', '')
                    })

    print(f"共找到 {len(results)} 条跨学科词项。\n")
    print(f"{ '词组':<10} | {'拼音':<12} | {'学科分类'}")
    print("-" * 60)
    
    # 打印前 50 条作为示例
    for item in results[:50]:
        print(f"{item['char']:<10} | {item['pinyin']:<12} | {item['category']}")
    
    if len(results) > 50:
        print("\n... 还有更多 ...")

if __name__ == "__main__":
    find_words_with_multiple_categories("dicts/chinese/vocabulary/words.json")