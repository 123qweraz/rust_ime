use crate::keys::VirtualKey;
use crate::processor::Action;
use crate::scheme::{InputScheme, SchemeCandidate, SchemeContext};

pub struct JapaneseScheme;

impl JapaneseScheme {
    pub fn new() -> Self {
        Self
    }
}

impl InputScheme for JapaneseScheme {
    fn name(&self) -> &str {
        "japanese"
    }

    fn lookup(&self, query: &str, context: &SchemeContext) -> Vec<SchemeCandidate> {
        let mut results = Vec::new();
        if let Some(trie) = context.tries.get("japanese") {
            // 1. 精确匹配 (主要是假名和单词)
            if let Some(matches) = trie.get_all_exact(query) {
                for tr in matches {
                    let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                    cand.traditional = tr.trad.to_string();
                    cand.tone = tr.tone.to_string();
                    cand.english = tr.en.to_string();
                    cand.stroke_aux = tr.stroke_aux.to_string();
                    cand.match_level = 3;
                    results.push(cand);
                }
            }

            // 2. 前缀匹配
            if context.config.input.enable_prefix_matching {
                let limit = if query.len() > 3 { 5 } else { 20 };
                let matches = trie.search_bfs(query, limit);
                for tr in matches {
                    let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                    cand.traditional = tr.trad.to_string();
                    cand.tone = tr.tone.to_string();
                    cand.english = tr.en.to_string();
                    cand.stroke_aux = tr.stroke_aux.to_string();
                    cand.match_level = 1;
                    results.push(cand);
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
        // 日语方案按匹配级别和权重排序
        candidates.sort_by(|a, b| {
            b.match_level
                .cmp(&a.match_level)
                .then_with(|| b.weight.cmp(&a.weight))
        });

        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.text.clone()));
    }

    fn handle_special_key(
        &self,
        key: VirtualKey,
        buffer: &mut String,
        _context: &SchemeContext,
    ) -> Option<Action> {
        // 处理日语专用标点符号映射
        // 只有在缓冲区为空时，标点符号才可能通过 handle_direct -> handle_punctuation 走通用逻辑
        // 在缓冲区不为空时，我们需要拦截这些按键实现上屏并附带标点

        let _shift = false; // 简化处理，主要处理非 shift 情况

        let punc = match key {
            VirtualKey::Dot => Some("。"),
            VirtualKey::Comma => Some("、"),
            VirtualKey::Slash => Some("・"),
            VirtualKey::LeftBrace => Some("「"),
            VirtualKey::RightBrace => Some("」"),
            VirtualKey::Minus => Some("ー"), // 日语长音符
            _ => None,
        };

        if let Some(_p) = punc {
            // 如果缓冲区有内容，这里逻辑较复杂，暂时返回 None 让 Processor 通用逻辑处理
            // 但对于日语模式，我们希望直接映射
            // 修正：如果 buffer 为空，我们在通用逻辑处理；如果 buffer 不为空，这里拦截并处理上屏
            if !buffer.is_empty() {
                // 这里的处理逻辑需要与 Processor 的 handle_punctuation 保持一致
                // 简单起见，我们暂不在此处实现复杂的“带缓冲上屏”，
                // 而是由 Processor 调用的通用 handle_punctuation 来处理。
                // 我们只需要确保 Processor 能获取到日语模式下的正确映射。
            }
        }

        None
    }
}
