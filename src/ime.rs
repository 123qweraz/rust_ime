use std::collections::HashMap;

pub enum ImeOutput {
    Commit(String),
    None,
}

pub struct ImeState {
    pub buffer: String,
    pub dict: HashMap<String, Vec<String>>,
    pub chinese: bool,
}

impl ImeState {
    pub fn new(dict: HashMap<String, Vec<String>>) -> Self {
        Self {
            buffer: String::new(),
            dict,
            chinese: false,
        }
    }

    pub fn toggle(&mut self) {
        self.chinese = !self.chinese;
        self.buffer.clear();
    }

    /// Returns:
    /// - Some(ImeOutput) => Output generated to be sent to VKBD
    /// - None           => Pass through original key
    pub fn handle_char(&mut self, c: char) -> Option<ImeOutput> {
        if !self.chinese {
            return None;
        }

        // Only buffer a-z for now
        if c.is_ascii_alphabetic() {
             self.buffer.push(c.to_ascii_lowercase());
             // In a real app, we'd emit an update event here for UI
             return Some(ImeOutput::None); // Swallow key
        }
        
        None
    }

    pub fn backspace(&mut self) -> Option<ImeOutput> {
        if !self.chinese || self.buffer.is_empty() {
            return None;
        }
        self.buffer.pop();
        Some(ImeOutput::None) // Swallow backspace
    }

    pub fn commit(&mut self) -> Option<ImeOutput> {
        if self.buffer.is_empty() {
             return None;
        }
        
        // Simple strategy: First candidate or raw pinyin
        let out = if let Some(cands) = self.dict.get(&self.buffer) {
            cands[0].clone()
        } else {
            self.buffer.clone()
        };
        
        self.buffer.clear();
        Some(ImeOutput::Commit(out))
    }
    
    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}
