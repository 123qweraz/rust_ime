#[derive(Clone, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub trait InputMethodHost {
    fn set_preedit(&self, text: &str, cursor_pos: usize);
    fn commit_text(&self, text: &str);
    fn get_cursor_rect(&self) -> Option<Rect>;
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}