#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import json
import os
import re
import math
import zipfile
from collections import Counter
from pypinyin import lazy_pinyin


class NewWordDiscoveryV3:
    def __init__(self, dict_dir="."):
        self.known_words = self._load_known_words(dict_dir)
        self.min_count = 5
        self.min_pmi = 4.0
        self.min_entropy = 0.8
        # 边界停用词：不应出现在词首或词尾的虚词
        self.boundary_stopwords = set("的了和是在有而及与或之为其于以到等说着也就都吧呢吗啊呀让把给被")

    def _load_known_words(self, dict_dir):
        known = set()
        if not os.path.exists(dict_dir):
            return known

        # 优先加载核心字典文件
        core_dicts = ["dict.json", "dict_cizu.json"]
        for d in core_dicts:
            d_path = os.path.join(dict_dir, d)
            if os.path.exists(d_path):
                self._load_from_json(d_path, known)

        # 也可以加载其他 json，但排除掉我们自己生成的发现词文件
        for root, dirs, files in os.walk(dict_dir):
            # 排除生成的文件夹
            if "new_words" in root or "new_words_v3" in root:
                continue
            for file in files:
                if file.endswith(".json") and file not in core_dicts:
                    # 进一步排除可能的输出文件
                    if "discovered" in file or "new_words" in file:
                        continue
                    self._load_from_json(os.path.join(root, file), known)
        return known

    def _load_from_json(self, file_path, known_set):
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                data = json.load(f)
                if isinstance(data, dict):
                    for val in data.values():
                        if isinstance(val, list):
                            for item in val:
                                if isinstance(item, dict) and "char" in item:
                                    known_set.add(item["char"])
                                elif isinstance(item, str):
                                    known_set.add(item)
                        elif isinstance(val, str):
                            known_set.add(val)
        except:
            pass

    def _read_content(self, file_path):
        ext = os.path.splitext(file_path)[1].lower()

        if ext == ".txt":
            try:
                with open(file_path, "r", encoding="utf-8", errors="ignore") as f:
                    return f.read()
            except Exception as e:
                return f"错误: 读取文件失败 - {str(e)}"

        elif ext == ".pdf":
            try:
                from pdfminer.high_level import extract_text
                return extract_text(file_path)
            except ImportError:
                return "错误: 请安装 pdfminer.six (pip install pdfminer.six)"
            except Exception as e:
                return f"错误: 读取 PDF 失败 - {str(e)}"

        elif ext == ".epub":
            content = []
            try:
                with zipfile.ZipFile(file_path, "r") as z:
                    for name in z.namelist():
                        if name.endswith((".html", ".xhtml", ".htm")):
                            with z.open(name) as f:
                                html = f.read().decode("utf-8", errors="ignore")
                                text = re.sub(r"<[^>]+>", "", html)
                                content.append(text)
                return "\n".join(content)
            except Exception as e:
                return f"错误: 解析 ePub 失败 - {str(e)}"

        return ""

    def _compute_entropy(self, counts):
        if not counts:
            return 0.0
        total = sum(counts)
        if total == 0:
            return 0.0
        entropy = 0.0
        for c in counts:
            p = c / total
            entropy -= p * math.log2(p)
        return entropy

    def _get_pinyin(self, word):
        try:
            py = lazy_pinyin(word)
            return "".join(py)
        except:
            return None

    def extract(self, file_path, max_word_len=4):
        text = self._read_content(file_path)
        if not text or text.startswith("错误"):
            print(text or "读取文件为空")
            return {}

        print("正在预处理文本...")
        # 仅保留汉字
        sentences = re.split(r"[^一-龥]+", text)
        sentences = [s for s in sentences if len(s) > 1]

        total_len = sum(len(s) for s in sentences)
        if total_len == 0:
            print("未提取到有效中文内容。 ולא")
            return {}

        print(f"正在分析 {total_len} 个汉字...")

        # 统计 N-gram，为了计算 max_word_len 的熵，我们需要统计到 max_word_len + 1
        ngrams = Counter()
        for sent in sentences:
            slen = len(sent)
            for n in range(1, max_word_len + 2):
                for i in range(slen - n + 1):
                    ngrams[sent[i : i + n]] += 1

        right_neighbor_counts = {}
        left_neighbor_counts = {}

        for word, count in ngrams.items():
            wlen = len(word)
            if wlen < 2:
                continue

            # word = prefix + last_char
            prefix = word[:-1]
            if prefix not in right_neighbor_counts:
                right_neighbor_counts[prefix] = []
            right_neighbor_counts[prefix].append(count)

            # word = first_char + suffix
            suffix = word[1:]
            if suffix not in left_neighbor_counts:
                left_neighbor_counts[suffix] = []
            left_neighbor_counts[suffix].append(count)

        results = {}
        print("正在计算指标并筛选词汇...")

        for word, count in ngrams.items():
            wlen = len(word)
            if wlen < 2 or wlen > max_word_len:
                continue

            if count < self.min_count:
                continue

            # 1. 已知词过滤
            if word in self.known_words:
                continue

            # 2. 边界停用词过滤 (核心改进)
            if word[0] in self.boundary_stopwords or word[-1] in self.boundary_stopwords:
                continue
            
            # 3. 包含已知词片段过滤 (可选，但能有效减少 "的危害" 类，如果 "危害" 已在词库中)
            # 如果 word = 停用词 + 已知词 或 已知词 + 停用词，则过滤
            is_redundant = False
            for k in range(1, wlen):
                part1, part2 = word[:k], word[k:]
                if (part1 in self.known_words and part2 in self.boundary_stopwords) or \
                   (part2 in self.known_words and part1 in self.boundary_stopwords):
                    is_redundant = True
                    break
            if is_redundant:
                continue

            # 4. 计算凝固度 (PMI)
            min_pmi = float("inf")
            p_word = count / total_len
            for k in range(1, wlen):
                part1 = word[:k]
                part2 = word[k:]
                c1 = ngrams.get(part1, 0)
                c2 = ngrams.get(part2, 0)
                if c1 > 0 and c2 > 0:
                    p1 = c1 / total_len
                    p2 = c2 / total_len
                    pmi = math.log2(p_word / (p1 * p2))
                    if pmi < min_pmi:
                        min_pmi = pmi
            
            if min_pmi < self.min_pmi:
                continue

            # 5. 计算自由度 (Boundary Entropy)
            r_entropy = self._compute_entropy(right_neighbor_counts.get(word, []))
            l_entropy = self._compute_entropy(left_neighbor_counts.get(word, []))
            min_entropy_val = min(r_entropy, l_entropy)
            
            if min_entropy_val < self.min_entropy:
                continue

            results[word] = {
                "count": count,
                "pmi": round(min_pmi, 2),
                "entropy": round(min_entropy_val, 2),
            }

        # 按词频排序
        sorted_results = dict(
            sorted(results.items(), key=lambda x: x[1]["count"], reverse=True)
        )
        return sorted_results

    def save_to_dictcizu_format(self, results, output_path):
        dictcuzu_format = {}
        for word in results.keys():
            pinyin = self._get_pinyin(word)
            if pinyin:
                if pinyin not in dictcuzu_format:
                    dictcuzu_format[pinyin] = []
                dictcuzu_format[pinyin].append({"char": word, "en": ""})

        sorted_dictcuzu = dict(sorted(dictcuzu_format.items()))
        try:
            with open(output_path, "w", encoding="utf-8") as f:
                json.dump(sorted_dictcuzu, f, ensure_ascii=False, indent=2)
            print(f"提取完成！发现 {len(results)} 个新词，已保存至: {output_path}")
            return sorted_dictcuzu
        except Exception as e:
            print(f"保存失败: {e}")
            return {}

    def save_to_txt_format(self, results, output_path):
        try:
            with open(output_path, "w", encoding="utf-8") as f:
                for word in results.keys():
                    f.write(f"{word}\n")
            print(f"TXT 格式保存完成：{output_path}")
        except Exception as e:
            print(f"保存 TXT 失败: {e}")

    def process_file(self, file_path, output_dir="new_words_v3"):
        results = self.extract(file_path)
        if not results:
            return None
        if not os.path.exists(output_dir):
            os.makedirs(output_dir)
        filename = os.path.basename(file_path)
        name_without_ext = os.path.splitext(filename)[0]
        
        # 保存 JSON
        json_path = os.path.join(output_dir, f"{name_without_ext}_new_words.json")
        self.save_to_dictcizu_format(results, json_path)
        
        # 保存 TXT
        txt_path = os.path.join(output_dir, f"{name_without_ext}_new_words.txt")
        self.save_to_txt_format(results, txt_path)
        
        return results

    def batch_process(self, root_dir, output_dir="new_words_v3"):
        print("=" * 60)
        print("新词批量提取工具 V3 (优化版)")
        print("=" * 60)

        if not os.path.exists(root_dir):
            print(f"错误: 目录不存在 - {root_dir}")
            return {}

        if not os.path.exists(output_dir):
            os.makedirs(output_dir)

        all_results = {}
        subject_stats = {}
        total_files = 0

        for root, dirs, files in os.walk(root_dir):
            # 过滤隐藏目录
            dirs[:] = [d for d in dirs if not d.startswith(".")]
            
            subject_name = os.path.basename(root)
            if not subject_name or subject_name == os.path.basename(root_dir):
                subject_name = "其他"

            for file in files:
                if file.lower().endswith((".txt", ".pdf")):
                    total_files += 1
                    file_path = os.path.join(root, file)
                    print(f"\n正在处理: {file_path}")

                    results = self.extract(file_path)
                    if results:
                        if subject_name not in subject_stats:
                            subject_stats[subject_name] = {"files": 0, "words": 0}
                        subject_stats[subject_name]["files"] += 1
                        subject_stats[subject_name]["words"] += len(results)

                        # 保存单文件结果
                        filename = os.path.basename(file_path)
                        name_without_ext = os.path.splitext(filename)[0]
                        self.save_to_dictcizu_format(results, os.path.join(output_dir, f"{name_without_ext}_new_words.json"))
                        self.save_to_txt_format(results, os.path.join(output_dir, f"{name_without_ext}_new_words.txt"))

                        # 合并到总结果
                        for word, info in results.items():
                            if word not in all_results or info['count'] > all_results[word]['count']:
                                all_results[word] = info

        # 汇总保存
        sorted_all = dict(sorted(all_results.items(), key=lambda x: x[1]['count'], reverse=True))
        self.save_to_dictcizu_format(sorted_all, os.path.join(output_dir, "all_discovered_v3.json"))
        self.save_to_txt_format(sorted_all, os.path.join(output_dir, "all_discovered_v3.txt"))

        print("\n" + "=" * 60)
        print("提取完成!")
        print("=" * 60)
        print(f"总文件数: {total_files}")
        print(f"总发现新词: {len(all_results)}")

        print("\n按科目统计:")
        for subject, stats in sorted(subject_stats.items()):
            print(f"- {subject}: {stats['files']} 个文件，发现 {stats['words']} 个新词")
        
        return all_results


def main():
    import sys
    if len(sys.argv) < 2:
        print("用法:")
        print("  批量提取: python3 word_discovery_v3.py <目录>")
        print("  单文件提取: python3 word_discovery_v3.py <文件>")
        sys.exit(1)

    discovery = NewWordDiscoveryV3(".")
    target = sys.argv[1]

    if os.path.isfile(target):
        discovery.process_file(target)
    elif os.path.isdir(target):
        discovery.batch_process(target)
    else:
        print(f"错误: 路径不存在 - {target}")

if __name__ == "__main__":
    main()
