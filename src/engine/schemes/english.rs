use crate::engine::keys::VirtualKey;
use crate::engine::processor::Action;
use crate::engine::scheme::{InputScheme, SchemeCandidate, SchemeContext};

pub struct EnglishScheme;

impl EnglishScheme {
    pub fn new() -> Self {
        Self
    }
}

impl InputScheme for EnglishScheme {
    fn name(&self) -> &str {
        "english"
    }

    fn lookup(&self, query: &str, context: &SchemeContext) -> Vec<SchemeCandidate> {
        let mut results = Vec::new();
        if let Some(trie) = context.tries.get("english") {
            // 1. 精确匹配
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
        // 英语方案通常按权重排序即可
        candidates.sort_by(|a, b| {
            b.match_level
                .cmp(&a.match_level)
                .then_with(|| b.weight.cmp(&a.weight))
        });

        // 去重
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.text.clone()));
    }

    fn handle_special_key(
        &self,
        _key: VirtualKey,
        _buffer: &mut String,
        _context: &SchemeContext,
    ) -> Option<Action> {
        None
    }
}
