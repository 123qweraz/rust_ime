use crate::engine::Processor;
use crate::platform::traits::{InputMethodHost, Rect};
use crate::ui::GuiEvent;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct WaylandHost {}

impl WaylandHost {
    pub fn new(_processor: Arc<Mutex<Processor>>, _gui_tx: Option<Sender<GuiEvent>>) -> Self { Self {} }
}

impl InputMethodHost for WaylandHost {
    fn set_preedit(&self, _: &str, _: usize) {}
    fn commit_text(&self, _: &str) {}
    fn get_cursor_rect(&self) -> Option<Rect> { None }
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[WaylandHost] 原生协议模式开发已挂起，请查看 FUTURE_ROADMAP.md");
        loop { std::thread::sleep(std::time::Duration::from_secs(3600)); }
    }
}
