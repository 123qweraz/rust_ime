use std::collections::HashMap;

pub enum ImeOutput {
    Commit(String),
    None,
}

pub struct ImeState {
    pub buffer: String,
    pub dict: HashMap<String, Vec<String>>,
    pub chinese: bool,
    pub candidates: Vec<String>,
    pub page_index: usize,
}

const PAGE_SIZE: usize = 5;

impl ImeState {
    pub fn new(dict: HashMap<String, Vec<String>>) -> Self {
        Self {
            buffer: String::new(),
            dict,
            chinese: false,
            candidates: Vec::new(),
            page_index: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.chinese = !self.chinese;
        self.reset();
    }

    pub fn get_current_page(&self) -> &[String] {
        let start = self.page_index * PAGE_SIZE;
        let end = (start + PAGE_SIZE).min(self.candidates.len());
        if start >= self.candidates.len() {
            &[]
        } else {
            &self.candidates[start..end]
        }
    }

    pub fn next_page(&mut self) -> bool {
        if (self.page_index + 1) * PAGE_SIZE < self.candidates.len() {
            self.page_index += 1;
            true
        } else {
            false
        }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.page_index > 0 {
            self.page_index -= 1;
            true
        } else {
            false
        }
    }

    pub fn select_candidate(&mut self, index: usize) -> Option<String> {
        let actual_index = self.page_index * PAGE_SIZE + index;
        if actual_index < self.candidates.len() {
            let res = self.candidates[actual_index].clone();
            self.reset();
            Some(res)
        } else {
            None
        }
    }

    pub fn handle_char(&mut self, c: char) -> Option<ImeOutput> {
        if !self.chinese {
            return None;
        }

        if c.is_ascii_alphabetic() {
            self.buffer.push(c.to_ascii_lowercase());
            self.update_candidates();
            return Some(ImeOutput::None);
        }
        
        None
    }

    fn update_candidates(&mut self) {
        self.candidates = self.dict.get(&self.buffer).cloned().unwrap_or_default();
        self.page_index = 0;
    }

    pub fn backspace(&mut self) -> Option<ImeOutput> {
        if !self.chinese || self.buffer.is_empty() {
            return None;
        }
        self.buffer.pop();
        self.update_candidates();
        Some(ImeOutput::None)
    }

    pub fn commit(&mut self) -> Option<ImeOutput> {
        if self.buffer.is_empty() {
             return None;
        }
        
        let out = if !self.candidates.is_empty() {
            self.candidates[0].clone()
        } else {
            self.buffer.clone()
        };
        
        self.reset();
        Some(ImeOutput::Commit(out))
    }
    
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.page_index = 0;
    }
}
