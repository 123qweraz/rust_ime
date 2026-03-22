use crate::pipeline::Candidate;
use crate::processor::{FilterMode, ImeState};

#[derive(Debug, Clone)]
pub struct InputSession {
    pub buffer: String,
    pub candidates: Vec<Candidate>,
    pub selected: usize,
    pub page: usize,
    pub cursor_pos: usize,
    pub joined_sentence: String,
    pub last_lookup_pinyin: String,
    pub state: ImeState,
    pub nav_mode: bool,
    pub switch_mode: bool,
    pub aux_filter: String,
    pub filter_mode: FilterMode,
    pub page_snapshot: Vec<Candidate>,
    pub shift_used_as_modifier: bool,

    pub best_segmentation: Vec<String>,
    pub phantom_text: String,
    pub preview_selected_candidate: bool,
    pub last_blocked_buffer: String,
    pub has_dict_match: bool,

    pub quote_open: bool,
    pub single_quote_open: bool,
}

impl InputSession {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            candidates: Vec::new(),
            selected: 0,
            page: 0,
            cursor_pos: 0,
            joined_sentence: String::new(),
            last_lookup_pinyin: String::new(),
            state: ImeState::Idle,
            nav_mode: false,
            switch_mode: false,
            aux_filter: String::new(),
            filter_mode: FilterMode::None,
            page_snapshot: Vec::new(),
            shift_used_as_modifier: false,

            best_segmentation: Vec::new(),
            phantom_text: String::new(),
            preview_selected_candidate: false,
            last_blocked_buffer: String::new(),
            has_dict_match: false,

            quote_open: false,
            single_quote_open: false,
        }
    }

    pub fn reset(&mut self) {
        self.clear_composing();
        self.switch_mode = false;
        self.quote_open = false;
        self.single_quote_open = false;
    }

    pub fn clear_composing(&mut self) {
        self.buffer.clear();
        self.candidates.clear();
        self.best_segmentation.clear();
        self.joined_sentence.clear();
        self.selected = 0;
        self.page = 0;
        self.state = ImeState::Idle;

        self.phantom_text.clear();
        self.preview_selected_candidate = false;
        self.cursor_pos = 0;
        self.aux_filter.clear();
        self.filter_mode = FilterMode::None;
        self.page_snapshot.clear();
        self.nav_mode = false;
    }

    pub fn push_char(&mut self, c: char) {
        self.buffer.push(c);
        if self.state == ImeState::Idle {
            self.state = ImeState::Composing;
        }
        self.preview_selected_candidate = false;
    }

    pub fn pop_char(&mut self) -> bool {
        if self.buffer.is_empty() {
            return false;
        }
        self.buffer.pop();
        if self.buffer.is_empty() {
            self.reset();
        }
        true
    }

    pub fn next_candidate(&mut self, page_size: usize) {
        if !self.candidates.is_empty() {
            self.preview_selected_candidate = true;
            if self.selected + 1 < self.candidates.len() {
                self.selected += 1;
            }
            self.page = (self.selected / page_size) * page_size;
        }
    }

    pub fn prev_candidate(&mut self, page_size: usize) {
        if !self.candidates.is_empty() {
            self.preview_selected_candidate = true;
            if self.selected > 0 {
                self.selected -= 1;
            }
            self.page = (self.selected / page_size) * page_size;
        }
    }

    pub fn next_page(&mut self, page_size: usize) {
        if !self.candidates.is_empty() && self.page + page_size < self.candidates.len() {
            self.page += page_size;
            self.selected = self.page;
        }
    }

    pub fn prev_page(&mut self, page_size: usize) {
        self.page = self.page.saturating_sub(page_size);
        self.selected = self.page;
    }

    pub fn handle_filter_char(&mut self, c: char) {
        if self.filter_mode == FilterMode::None {
            self.filter_mode = FilterMode::Page;
            self.page_snapshot = self.candidates.clone();
            self.aux_filter = c.to_string();
        } else {
            self.aux_filter.push(c);
        }
        self.selected = 0;
        if self.filter_mode == FilterMode::Global {
            self.page = 0;
        }
    }

    pub fn pop_filter(&mut self) {
        if !self.aux_filter.is_empty() {
            self.aux_filter.pop();
            if self.aux_filter.is_empty() {
                self.filter_mode = FilterMode::None;
                self.page_snapshot.clear();
                self.page = 0;
            } else {
                self.selected = 0;
                if self.filter_mode == FilterMode::Global {
                    self.page = 0;
                }
            }
        }
    }

    pub fn update_state(&mut self) {
        if self.buffer.is_empty() {
            self.state = ImeState::Idle;
        } else if self.state == ImeState::Idle {
            self.state = ImeState::Composing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::ImeState;

    #[test]
    fn test_session_basic_ops() {
        let mut session = InputSession::new();
        assert_eq!(session.state, ImeState::Idle);

        session.push_char('n');
        assert_eq!(session.buffer, "n");
        assert_eq!(session.state, ImeState::Composing);

        session.pop_char();
        assert_eq!(session.buffer, "");
        assert_eq!(session.state, ImeState::Idle);
    }

    #[test]
    fn test_session_state_updates() {
        let mut session = InputSession::new();
        session.buffer = "nh".to_string();
        session.update_state();
        assert_eq!(session.state, ImeState::Composing);

        session.buffer.clear();
        session.update_state();
        assert_eq!(session.state, ImeState::Idle);
    }

    #[test]
    fn test_session_reset() {
        let mut session = InputSession::new();
        session.push_char('a');
        session.nav_mode = true;
        session.reset();
        assert!(session.buffer.is_empty());
        assert!(!session.nav_mode);
        assert_eq!(session.state, ImeState::Idle);
    }

    #[test]
    fn test_session_push_pop() {
        let mut session = InputSession::new();
        session.push_char('a');
        session.push_char('b');
        session.push_char('c');
        assert_eq!(session.buffer, "abc");

        session.pop_char();
        assert_eq!(session.buffer, "ab");

        session.pop_char();
        session.pop_char();
        assert_eq!(session.buffer, "");
    }

    #[test]
    fn test_session_page_navigation() {
        let mut session = InputSession::new();
        session.candidates = vec![
            create_candidate("a"),
            create_candidate("b"),
            create_candidate("c"),
            create_candidate("d"),
            create_candidate("e"),
        ];

        session.selected = 2;
        session.next_candidate(3);
        assert_eq!(session.selected, 3);

        session.next_candidate(3);
        assert_eq!(session.selected, 4);

        session.prev_candidate(3);
        assert_eq!(session.selected, 3);
    }

    #[test]
    fn test_session_clear_composing() {
        let mut session = InputSession::new();
        session.buffer = "test".to_string();
        session.phantom_text = "best".to_string();
        session.clear_composing();
        assert!(session.buffer.is_empty());
        assert!(session.phantom_text.is_empty());
        assert_eq!(session.state, ImeState::Idle);
    }

    fn create_candidate(text: &str) -> crate::pipeline::Candidate {
        use std::sync::Arc;
        crate::pipeline::Candidate {
            text: Arc::from(text),
            simplified: Arc::from(text),
            traditional: Arc::from(text),
            hint: Arc::from(""),
            source: Arc::from("test"),
            weight: 1.0,
            match_level: 3,
        }
    }
}
