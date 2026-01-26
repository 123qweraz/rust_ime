use std::collections::{HashMap, VecDeque};

#[derive(Debug, Default, Clone)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    words: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Trie {
    root: TrieNode,
    total_words: usize,
}

impl Trie {
    pub fn new() -> Self {
        Self {
            root: TrieNode::default(),
            total_words: 0,
        }
    }

    pub fn insert(&mut self, pinyin: &str, word: String) {
        let mut node = &mut self.root;
        for c in pinyin.chars() {
            node = node.children.entry(c).or_default();
        }
        if !node.words.contains(&word) {
            node.words.push(word);
            self.total_words += 1;
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.total_words
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.total_words == 0
    }

    pub fn get_exact(&self, pinyin: &str) -> Option<String> {
        let mut node = &self.root;
        for c in pinyin.chars() {
            node = node.children.get(&c)?;
        }
        node.words.first().cloned()
    }

    pub fn get_all_exact(&self, pinyin: &str) -> Option<Vec<String>> {
        let mut node = &self.root;
        for c in pinyin.chars() {
            node = node.children.get(&c)?;
        }
        if node.words.is_empty() {
            None
        } else {
            Some(node.words.clone())
        }
    }

    /// Search for words starting with `prefix` using BFS.
    pub fn search_bfs(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut results = Vec::new();
        let mut node = &self.root;

        // 1. Navigate to the prefix node
        for c in prefix.chars() {
            match node.children.get(&c) {
                Some(n) => node = n,
                None => return results, // Prefix not found
            }
        }

        // 2. BFS from this node
        let mut queue = VecDeque::new();
        queue.push_back(node);

        while let Some(curr) = queue.pop_front() {
            for word in &curr.words {
                if !results.contains(word) {
                    results.push(word.clone());
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
            for child in curr.children.values() {
                queue.push_back(child);
            }
        }

        results
    }

    /// Fuzzy search using Levenshtein distance on the Trie.
    pub fn search_fuzzy(&self, pattern: &str, max_cost: usize) -> Vec<String> {
        let pattern_chars: Vec<char> = pattern.chars().collect();
        // The first row of the Levenshtein matrix: 0, 1, 2, ...
        let current_row: Vec<usize> = (0..=pattern_chars.len()).collect();
        
        let mut results = Vec::new();
        
        for (char, child) in &self.root.children {
            self.search_fuzzy_recursive(child, *char, &pattern_chars, &current_row, max_cost, &mut results);
        }
        
        results
    }

    fn search_fuzzy_recursive(
        &self,
        node: &TrieNode,
        char: char,
        pattern: &[char],
        prev_row: &[usize],
        max_cost: usize,
        results: &mut Vec<String>
    ) {
        let columns = pattern.len() + 1;
        let mut current_row = vec![0; columns];
        current_row[0] = prev_row[0] + 1;

        let mut min_val = current_row[0];

        for i in 1..columns {
            let insert_cost = current_row[i - 1] + 1;
            let delete_cost = prev_row[i] + 1;
            let replace_cost = prev_row[i - 1] + if pattern[i - 1] == char { 0 } else { 1 };

            current_row[i] = insert_cost.min(delete_cost).min(replace_cost);
            if current_row[i] < min_val {
                min_val = current_row[i];
            }
        }

        // Pruning: if the best possible match in this subtree is already worse than max_cost, stop.
        if min_val > max_cost {
            return;
        }

        // If the last entry in the row is within max_cost, this node matches the pattern closely enough.
        // Also, since this is an IME, we often want prefix matches too.
        // But for typo correction (gongnegn -> gongneng), we usually want full matches logic,
        // or at least "end of pattern matches current node".
        if current_row[pattern.len()] <= max_cost {
            for word in &node.words {
                if !results.contains(word) {
                    results.push(word.clone());
                }
            }
        }

        // Recurse
        for (next_char, next_child) in &node.children {
            self.search_fuzzy_recursive(next_child, *next_char, pattern, &current_row, max_cost, results);
        }
    }
}
