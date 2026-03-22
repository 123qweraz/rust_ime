use rust_ime_core::Rect;
use std::error::Error;

pub trait InputMethodHost: Send {
    fn set_preedit(&self, text: &str, cursor_pos: usize);
    fn commit_text(&self, text: &str);
    fn get_cursor_rect(&self) -> Option<Rect>;
    fn run(&mut self) -> Result<(), Box<dyn Error>>;
}
