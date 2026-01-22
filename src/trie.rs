use std::collections::{HashMap, VecDeque};

#[derive(Debug, Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    words: Vec<String>,
}

#[derive(Debug, Default)]
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

    pub fn len(&self) -> usize {
        self.total_words
    }

    pub fn is_empty(&self) -> bool {
        self.total_words == 0
    }

    /// Search for words starting with `prefix` using BFS.
    /// This ensures words associated with shorter pinyins (exact matches) appear first,
    /// followed by longer pinyin matches.
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
            // Add words from current node
            for word in &curr.words {
                if !results.contains(word) {
                    results.push(word.clone());
                    if results.len() >= limit {
                        return results;
                    }
                }
            }

            // Enqueue children
            // Note: HashMap iteration order is random. If we want stable ordering 
            // for pinyins of the same length (e.g. 'nia' vs 'nib'), we might need to sort keys.
            // But usually length is the primary factor for input methods.
            for child in curr.children.values() {
                queue.push_back(child);
            }
        }

        results
    }
}
