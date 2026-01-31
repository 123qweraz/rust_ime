use std::collections::HashSet;
use crate::engine::trie::Trie;

pub struct Segmenter {
    pub syllable_set: HashSet<String>,
}

impl Segmenter {
    pub fn new() -> Self {
        let mut syllable_set = HashSet::new();
        if let Ok(content) = std::fs::read_to_string("dicts/chinese/syllables.txt") {
            for line in content.lines() {
                let s = line.trim();
                if !s.is_empty() { syllable_set.insert(s.to_string()); }
            }
        }
        Self { syllable_set }
    }

    pub fn segment_all(&self, pinyin: &str, dict: &Trie) -> Vec<Vec<String>> {
        let mut results = Vec::new();
        let mut current = Vec::new();
        self.segment_recursive(pinyin, dict, &mut current, &mut results);
        if results.is_empty() {
            results.push(self.segment_greedy(pinyin, dict));
        }
        results
    }

    fn segment_recursive(&self, remaining: &str, dict: &Trie, current: &mut Vec<String>, results: &mut Vec<Vec<String>>) {
        if remaining.is_empty() { results.push(current.clone()); return; }
        if results.len() >= 15 { return; }

        let has_apostrophe = remaining.starts_with('`');
        if has_apostrophe {
            let actual = &remaining[1..];
            let max_len = actual.len().min(6);
            for len in (1..=max_len).rev() {
                let sub = &actual[..len];
                if self.syllable_set.contains(sub) || dict.contains(sub) {
                    current.push(sub.to_string());
                    self.segment_recursive(&actual[len..], dict, current, results);
                    current.pop();
                    if results.len() >= 15 { return; }
                }
            }
            return;
        }

        let max_len = remaining.len().min(6);
        for len in (2..=max_len).rev() {
            let sub = &remaining[..len];
            if self.syllable_set.contains(sub) || dict.contains(sub) {
                current.push(sub.to_string());
                self.segment_recursive(&remaining[len..], dict, current, results);
                current.pop();
                if results.len() >= 15 { return; }
            }
        }
        if !remaining.is_empty() {
            let sub = &remaining[..1];
            current.push(sub.to_string());
            self.segment_recursive(&remaining[1..], dict, current, results);
            current.pop();
        }
    }

    fn segment_greedy(&self, pinyin: &str, dict: &Trie) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_offset = 0;
        while current_offset < pinyin.len() {
            let mut found_len = 0;
            let current_str = &pinyin[current_offset..];
            if current_str.starts_with('`') { current_offset += 1; continue; }
            for len in (1..=current_str.len().min(6)).rev() {
                let sub = &current_str[..len];
                if dict.contains(sub) || self.syllable_set.contains(sub) { found_len = len; break; }
            }
            if found_len > 0 { segments.push(current_str[..found_len].to_string()); current_offset += found_len; }
            else { segments.push(current_str[..1].to_string()); current_offset += 1; }
        }
        segments
    }
}
