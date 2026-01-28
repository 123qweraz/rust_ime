use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter};
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct NgramModel {
    // context (1 to 3 chars) -> (next_char -> frequency)
    // Key 为 String，可以支持变长上下文
    pub transitions: HashMap<String, HashMap<char, u32>>,
    pub max_n: usize,
}

impl NgramModel {
    pub fn new() -> Self {
        Self {
            transitions: HashMap::new(),
            max_n: 4, // 默认支持到 4-gram (3个字的上下文预测第4个字)
        }
    }

    pub fn train(&mut self, text: &str) {
        // 将文本按换行符和标点符号切分成独立的片段
        let sections = text.split(|c: char| {
            c == '\n' || c == '\r' || c == '。' || c == '，' || c == '！' || c == '？' || c == '；' || c == '：' || c == '“' || c == '”' || c == '（' || c == '）' || c == '、'
        });

        for section in sections {
            // 对每个片段提取汉字
            let chars: Vec<char> = section.chars()
                .filter(|c| {
                    (*c >= '\u{4e00}' && *c <= '\u{9fa5}') ||
                    (*c >= '\u{3400}' && *c <= '\u{4dbf}') ||
                    (*c >= '\u{20000}' && *c <= '\u{2a6df}')
                })
                .collect();

            if chars.len() < 2 {
                continue;
            }

            // 在片段内部学习关联
            for n in 2..=self.max_n {
                if chars.len() < n { continue; }
                for window in chars.windows(n) {
                    let context: String = window[..n-1].iter().collect();
                    let next_char = window[n-1];

                    let entry = self.transitions.entry(context).or_default();
                    *entry.entry(next_char).or_default() += 1;
                }
            }
        }
    }

    pub fn update(&mut self, context_chars: &[char], next_char: char) {
        // 实时更新 2-gram 到 max_n-gram
        for len in 1..self.max_n {
            if context_chars.len() < len { break; }
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();
            
            let entry = self.transitions.entry(context).or_default();
            *entry.entry(next_char).or_default() += 1;
        }
    }

    pub fn predict(&self, context_chars: &[char], limit: usize) -> Vec<String> {
        // 实现 Back-off 逻辑：从最长上下文开始找
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                let mut candidates: Vec<(&char, &u32)> = next_map.iter().collect();
                candidates.sort_by(|a, b| b.1.cmp(a.1));
                
                return candidates.into_iter()
                    .take(limit)
                    .map(|(c, _)| c.to_string())
                    .collect();
            }
        }
        Vec::new()
    }

    pub fn get_score(&self, context_chars: &[char], next_char: char) -> u32 {
        // 同样实现 Back-off 打分
        for len in (1..=context_chars.len().min(self.max_n - 1)).rev() {
            let start = context_chars.len() - len;
            let context: String = context_chars[start..].iter().collect();

            if let Some(next_map) = self.transitions.get(&context) {
                if let Some(&score) = next_map.get(&next_char) {
                    // 越长的匹配分数权重越高
                    return score * (len as u32);
                }
            }
        }
        0
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