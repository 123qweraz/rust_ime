use crate::keys::VirtualKey;
use crate::processor::Action;
use crate::scheme::{InputScheme, SchemeCandidate, SchemeContext};

pub struct StrokeScheme;

impl StrokeScheme {
    pub fn new() -> Self {
        Self
    }

    /// 将 1-5 数字序列转为字母编码 (双笔一键逻辑)
    fn encode_stroke(&self, s: &str) -> String {
        let mut res = String::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if i + 1 < chars.len() {
                let pair = format!("{}{}", chars[i], chars[i + 1]);
                let code = match pair.as_str() {
                    "11" => 'g',
                    "12" => 'f',
                    "13" => 'd',
                    "14" => 's',
                    "15" => 'a',
                    "21" => 'h',
                    "22" => 'j',
                    "23" => 'k',
                    "24" => 'l',
                    "25" => 'm',
                    "31" => 't',
                    "32" => 'r',
                    "33" => 'e',
                    "34" => 'w',
                    "35" => 'q',
                    "41" => 'y',
                    "42" => 'u',
                    "43" => 'i',
                    "44" => 'o',
                    "45" => 'p',
                    "51" => 'n',
                    "52" => 'b',
                    "53" => 'v',
                    "54" => 'c',
                    "55" => 'x',
                    _ => ' ',
                };
                if code != ' ' {
                    res.push(code);
                    i += 2;
                    continue;
                }
            }
            let code = match chars[i] {
                '1' => 'g',
                '2' => 'h',
                '3' => 't',
                '4' => 'y',
                '5' => 'n',
                c if c.is_ascii_lowercase() => c, // 允许直接输入映射后的字母
                _ => ' ',
            };
            if code != ' ' {
                res.push(code);
            }
            i += 1;
        }
        res
    }
}

impl InputScheme for StrokeScheme {
    fn name(&self) -> &str {
        "stroke"
    }

    fn pre_process(&self, buffer: &str, _context: &SchemeContext) -> String {
        // 如果输入包含数字，进行转码；否则保留原样（支持直接输入字母）
        if buffer.chars().any(|c| c.is_ascii_digit()) {
            self.encode_stroke(buffer)
        } else {
            buffer.to_string()
        }
    }

    fn lookup(&self, query: &str, context: &SchemeContext) -> Vec<SchemeCandidate> {
        let mut results = Vec::new();
        let has_wildcard = query.contains('z');

        // 使用合并后的笔画词库，支持按等级过滤
        if let Some(trie) = context.tries.get("stroke") {
            // 1. 如果包含通配符，使用搜索方法
            if has_wildcard {
                let matches = trie.search_wildcard(query, 50);
                for tr in matches {
                    let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                    cand.traditional = tr.trad.to_string();
                    cand.tone = tr.tone.to_string();
                    cand.english = tr.en.to_string();
                    cand.stroke_aux = tr.stroke_aux.to_string();
                    cand.match_level = 2; // 通配匹配设为 2
                    results.push(cand);
                }
            } else {
                // 2. 无通配符，执行常规精确 + 前缀匹配
                if let Some(matches) = trie.get_all_exact(query) {
                    for tr in matches {
                        let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                        cand.traditional = tr.trad.to_string();
                        cand.tone = tr.tone.to_string();
                        cand.english = tr.en.to_string();
                        cand.stroke_aux = tr.stroke_aux.to_string();
                        cand.match_level = 3; // 精确匹配设为 3
                        results.push(cand);
                    }
                }

                // 前缀匹配：
                // - 全局开启前缀匹配时，始终启用；
                // - 即使全局关闭，当当前输入没有精确命中时也启用兜底前缀匹配，
                //   避免笔画模式下出现“只有全码才有结果”的体验。
                let enable_prefix_fallback = results.is_empty();
                if context.config.input.enable_prefix_matching || enable_prefix_fallback {
                    let limit = context.config.input.prefix_matching_limit.min(50);
                    let matches = trie.search_bfs(query, limit);
                    for tr in matches {
                        let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                        cand.traditional = tr.trad.to_string();
                        cand.tone = tr.tone.to_string();
                        cand.english = tr.en.to_string();
                        cand.stroke_aux = tr.stroke_aux.to_string();
                        cand.match_level = 1; // 前缀匹配设为 1
                        results.push(cand);
                    }
                }
            }
        }
        results
    }

    fn post_process(
        &self,
        _query: &str,
        candidates: &mut Vec<SchemeCandidate>,
        _context: &SchemeContext,
    ) {
        // 按综合得分排序：级别基础分 + 匹配级别分 + 词频权重
        candidates.sort_by(|a, b| {
            let get_score = |c: &SchemeCandidate| -> i64 {
                // 1. 级别基础分（确保常用字优先）
                let cat_score = match c.stroke_aux.as_str() {
                    "level-1" => 1_000_000_000,
                    "level-2" => 500_000_000,
                    "level-3" => 200_000_000,
                    _ => 0,
                };

                // 2. 匹配级别分（精确匹配优先，但不要过度影响词频排序）
                // 精确匹配: 50,000,000
                // 前缀匹配: 10,000,000
                // 通配匹配: 0
                let level_score = match c.match_level {
                    3 => 50_000_000, // 精确匹配
                    1 => 10_000_000, // 前缀匹配
                    _ => 0,          // 通配匹配或其他
                };

                // 3. 词频权重（直接使用权重值）
                // 这样可以确保同一匹配级别内按词频排序
                let weight_score = c.weight as i64;

                cat_score + level_score + weight_score
            };
            get_score(b).cmp(&get_score(a))
        });

        // 去重（保留权重最高的）
        let mut seen = std::collections::HashMap::new();
        candidates.retain(|c| {
            let entry = seen.entry(c.text.clone());
            match entry {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(c.weight);
                    true
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    if c.weight > *e.get() {
                        e.insert(c.weight);
                        true
                    } else {
                        false
                    }
                }
            }
        });
    }

    fn handle_special_key(
        &self,
        key: VirtualKey,
        buffer: &mut String,
        context: &SchemeContext,
    ) -> Option<Action> {
        // 笔画模式下，1-5 数字优先作为输入，但如果有候选词，则优先选词
        if let Some(digit) = crate::processor::key_to_digit(key) {
            if (1..=5).contains(&digit) {
                // 如果当前已经有候选词了，我们返回 None，让 Processor 的通用选词逻辑去处理
                if context.candidate_count > 0 {
                    return None;
                }

                // 否则，将其作为笔画输入
                buffer.push_str(&digit.to_string());
                return Some(Action::Consume);
            }
        }
        None
    }
}
