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

    pub fn toggle_chinese(&mut self) {
        self.chinese_enabled = !self.chinese_enabled;
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

    pub fn get_last_pinyin(&self) -> Option<&str> {
        self.commit_history.last().map(|(p, _)| p.as_str())
    }

    pub fn clear_history(&mut self) {
        self.commit_history.clear();
    }

    pub fn should_clear_history(&self) -> bool {
        self.last_commit_time.elapsed().as_secs() > 3
    }

    pub fn update_commit_time(&mut self) {
        self.last_commit_time = Instant::now();
    }

    pub fn set_profiles(&mut self, profiles: Vec<String>) {
        self.active_profiles = profiles;
    }

    pub fn get_short_display(&self) -> String {
        let display = self.get_current_profile_display();
        match display.to_lowercase().as_str() {
            "chinese" => "中".to_string(),
            "english" => "英".to_string(),
            "japanese" => "日".to_string(),
            "stroke" => "笔".to_string(),
            "mixed" => "混".to_string(),
            _ => {
                let mut chars = display.chars();
                chars
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| " ".to_string())
            }
        }
    }

    pub fn get_current_profile_display(&self) -> String {
        if self.active_profiles.is_empty() {
            return "None".to_string();
        }
        if self.active_profiles.len() == 1 {
            return self.active_profiles[0].clone();
        }
        "Mixed".to_string()
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}
