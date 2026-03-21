use std::time::Instant;

#[derive(Debug, Clone)]
pub struct SessionState {
    pub active_profiles: Vec<String>,
    pub chinese_enabled: bool,
    pub commit_history: Vec<(String, String)>,
    pub last_commit_time: Instant,
    pub capslock_pending: bool,
    pub caps_lock_enabled: bool,
    pub capslock_down: bool,
}

impl SessionState {
    pub fn new() -> Self {
        Self {
            active_profiles: Vec::new(),
            chinese_enabled: true,
            commit_history: Vec::new(),
            last_commit_time: Instant::now(),
            capslock_pending: false,
            caps_lock_enabled: false,
            capslock_down: false,
        }
    }

    pub fn add_to_history(&mut self, pinyin: String, word: String) {
        self.commit_history.push((pinyin, word));
        if self.commit_history.len() > 10 {
            self.commit_history.remove(0);
        }
    }

    pub fn get_last_word(&self) -> Option<&str> {
        self.commit_history.last().map(|(_, w)| w.as_str())
    }

    pub fn update_commit_time(&mut self) {
        self.last_commit_time = Instant::now();
    }

    pub fn get_combination_candidates(&self, max_len: usize) -> Vec<(String, String)> {
        let start = if self.commit_history.len() > 4 {
            self.commit_history.len() - 4
        } else {
            0
        };
        let mut results = Vec::new();
        let history_slice = &self.commit_history[start..];

        for i in 0..history_slice.len() {
            let mut combined_py = String::new();
            let mut combined_word = String::new();
            for entry in &history_slice[i..] {
                combined_py.push_str(&entry.0);
                combined_word.push_str(&entry.1);
            }
            if combined_word.chars().count() <= max_len {
                results.push((combined_py, combined_word));
            }
        }
        results
    }

    pub fn get_current_profile(&self) -> String {
        self.active_profiles
            .first()
            .cloned()
            .unwrap_or_else(|| "chinese".to_string())
    }

    pub fn is_stroke_mode(&self) -> bool {
        self.active_profiles.contains(&"stroke".to_string())
    }

    pub fn is_english_mode(&self) -> bool {
        self.active_profiles.len() == 1 && self.active_profiles[0] == "english"
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}
