#!/usr/bin/env python3
import json
from pathlib import Path

def load_dict(file_path):
    """加载字典文件，返回汉字集合和笔画编码集合"""
    with open(file_path, 'r', encoding='utf-8') as f:
        data = json.load(f)

    chars = set()
    stroke_codes = set()

    for code_group in data.values():
        for entry in code_group:
            chars.add(entry['char'])
            stroke_codes.add(entry['strokes'])

    return chars, stroke_codes

def analyze_overlap():
    """分析三个级别字典的重合率"""
    base_path = Path('/home/x/Documents/rust_ime/dicts/stroke/chars')

    # 加载三个级别的字典
    level1_chars, level1_strokes = load_dict(base_path / 'stroke_chars_level-1.json')
    level2_chars, level2_strokes = load_dict(base_path / 'stroke_chars_level-2.json')
    level3_chars, level3_strokes = load_dict(base_path / 'stroke_chars_level-3.json')

    print("=" * 60)
    print("笔画词典级别重合率分析")
    print("=" * 60)

    # 统计基本信息
    print(f"\n[基本统计]")
    print(f"Level-1: {len(level1_chars)} 个汉字, {len(level1_strokes)} 个笔画编码")
    print(f"Level-2: {len(level2_chars)} 个汉字, {len(level2_strokes)} 个笔画编码")
    print(f"Level-3: {len(level3_chars)} 个汉字, {len(level3_strokes)} 个笔画编码")

    # 汉字级别的重合分析
    print(f"\n[汉字级别重合率]")

    l1_l2_chars = level1_chars & level2_chars
    l1_l3_chars = level1_chars & level3_chars
    l2_l3_chars = level2_chars & level3_chars
    l1_l2_l3_chars = level1_chars & level2_chars & level3_chars

    print(f"Level-1 ∩ Level-2: {len(l1_l2_chars)} 个汉字 ({len(l1_l2_chars)/len(level1_chars)*100:.2f}% of L1, {len(l1_l2_chars)/len(level2_chars)*100:.2f}% of L2)")
    print(f"Level-1 ∩ Level-3: {len(l1_l3_chars)} 个汉字 ({len(l1_l3_chars)/len(level1_chars)*100:.2f}% of L1, {len(l1_l3_chars)/len(level3_chars)*100:.2f}% of L3)")
    print(f"Level-2 ∩ Level-3: {len(l2_l3_chars)} 个汉字 ({len(l2_l3_chars)/len(level2_chars)*100:.2f}% of L2, {len(l2_l3_chars)/len(level3_chars)*100:.2f}% of L3)")
    print(f"Level-1 ∩ Level-2 ∩ Level-3: {len(l1_l2_l3_chars)} 个汉字")

    # 笔画编码级别的重合分析
    print(f"\n[笔画编码级别重合率]")

    l1_l2_strokes = level1_strokes & level2_strokes
    l1_l3_strokes = level1_strokes & level3_strokes
    l2_l3_strokes = level2_strokes & level3_strokes
    l1_l2_l3_strokes = level1_strokes & level2_strokes & level3_strokes

    print(f"Level-1 ∩ Level-2: {len(l1_l2_strokes)} 个笔画编码 ({len(l1_l2_strokes)/len(level1_strokes)*100:.2f}% of L1, {len(l1_l2_strokes)/len(level2_strokes)*100:.2f}% of L2)")
    print(f"Level-1 ∩ Level-3: {len(l1_l3_strokes)} 个笔画编码 ({len(l1_l3_strokes)/len(level1_strokes)*100:.2f}% of L1, {len(l1_l3_strokes)/len(level3_strokes)*100:.2f}% of L3)")
    print(f"Level-2 ∩ Level-3: {len(l2_l3_strokes)} 个笔画编码 ({len(l2_l3_strokes)/len(level2_strokes)*100:.2f}% of L2, {len(l2_l3_strokes)/len(level3_strokes)*100:.2f}% of L3)")
    print(f"Level-1 ∩ Level-2 ∩ Level-3: {len(l1_l2_l3_strokes)} 个笔画编码")

    # 总体统计
    total_chars = len(level1_chars | level2_chars | level3_chars)
    total_strokes = len(level1_strokes | level2_strokes | level3_strokes)

    print(f"\n[总体统计]")
    print(f"总汉字数: {total_chars} (去重后)")
    print(f"总笔画编码数: {total_strokes} (去重后)")
    print(f"汉字重复率: {(len(level1_chars) + len(level2_chars) + len(level3_chars) - total_chars) / (len(level1_chars) + len(level2_chars) + len(level3_chars)) * 100:.2f}%")
    print(f"笔画编码重复率: {(len(level1_strokes) + len(level2_strokes) + len(level3_strokes) - total_strokes) / (len(level1_strokes) + len(level2_strokes) + len(level3_strokes)) * 100:.2f}%")

if __name__ == '__main__':
    analyze_overlap()
