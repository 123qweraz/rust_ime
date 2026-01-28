use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NgramModel {
    pub transitions: HashMap<String, HashMap<String, u32>>,
    pub unigrams: HashMap<String, u32>,
    pub max_n: usize,
}

impl NgramModel {
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
            unigrams: HashMap::new(),
            max_n: 3,
        }
    }

    pub fn update(&mut self, context_chars: &[char], next_token: &str) {
        let token_str = next_token.to_string();
        *self.unigrams.entry(token_str.clone()).or_default() += 1;

        for len in 1..self.max_n {
            if context_chars.len() < len { break; }
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            let entry = self.transitions.entry(context).or_default();
            *entry.entry(token_str.clone()).or_default() += 1;
        }
    }

    pub fn predict(&self, context_chars: &[char], limit: usize) -> Vec<String> {
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                let mut candidates: Vec<(&String, &u32)> = next_map.iter().collect();
                candidates.sort_by(|a, b| b.1.cmp(a.1));
                
                return candidates.into_iter()
                    .take(limit)
                    .map(|(c, _)| c.clone())
                    .collect();
            }
        }
        Vec::new()
    }

    pub fn get_score(&self, context_chars: &[char], next_token_str: &str) -> u32 {
        let mut total_score = *self.unigrams.get(next_token_str).unwrap_or(&0);

        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                if let Some(&score) = next_map.get(next_token_str) {
                    total_score += score * 10 * (len as u32);
                    break; 
                }
            }
        }
        total_score
    }
    
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let model: Self = serde_json::from_reader(reader)?;
        Ok(model)
    }
}
