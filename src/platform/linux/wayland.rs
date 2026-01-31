use crate::engine::Processor;
use crate::platform::traits::{InputMethodHost, Rect};
use crate::ui::GuiEvent;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct WaylandHost {
    processor: Arc<Mutex<Processor>>,
    gui_tx: Option<Sender<GuiEvent>>,
}

impl WaylandHost {
    pub fn new(processor: Arc<Mutex<Processor>>, gui_tx: Option<Sender<GuiEvent>>) -> Self {
        Self {
            processor,
            gui_tx,
        }
    }
}

impl InputMethodHost for WaylandHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {}
    fn commit_text(&self, _text: &str) {}
    fn get_cursor_rect(&self) -> Option<Rect> { None }
    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        println!("[WaylandHost] 原生协议支持开发中，请使用 Evdev 模式。");
        loop { std::thread::sleep(std::time::Duration::from_secs(3600)); }
    }
}