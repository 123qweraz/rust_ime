use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BigramModel {
    // previous_char -> (next_char -> frequency)
    pub transitions: HashMap<char, HashMap<char, u32>>,
}

impl BigramModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn train(&mut self, text: &str) {
        let chars: Vec<char> = text.chars().collect();
        if chars.len() < 2 {
            return;
        }

        for window in chars.windows(2) {
            let prev = window[0];
            let next = window[1];

            // Filter: Ignore if either char is whitespace or control
            // We want to learn "汉"->"字", but maybe not "汉"->"\n"
            if prev.is_whitespace() || next.is_whitespace() || prev.is_control() || next.is_control() {
                continue;
            }

            let entry = self.transitions.entry(prev).or_default();
            *entry.entry(next).or_default() += 1;
        }
    }

    pub fn predict(&self, current_char: char, limit: usize) -> Vec<String> {
        if let Some(next_map) = self.transitions.get(&current_char) {
            let mut candidates: Vec<(&char, &u32)> = next_map.iter().collect();
            // Sort by frequency descending
            candidates.sort_by(|a, b| b.1.cmp(a.1));
            
            candidates.into_iter()
                .take(limit)
                .map(|(c, _)| c.to_string())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    pub fn save<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let model = serde_json::from_reader(reader)?;
        Ok(model)
    }
}
