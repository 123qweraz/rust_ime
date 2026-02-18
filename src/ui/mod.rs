pub mod tray;
pub mod web;
pub mod painter;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux as gui;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows as gui;

use crate::config::Config;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum GuiEvent {
    Update {
        pinyin: String,
        candidates: Vec<String>,
        hints: Vec<String>,
        selected: usize,
        sentence: String,
        cursor_pos: usize,
        commit_mode: String,
    },
    MoveTo { x: i32, y: i32 },
    Keystroke(String),
    ShowLearning(String, String), // 汉字, 提示
    HideLearning,
    ClearKeystrokes,
    ApplyConfig(Config),
        ShowStatus(String), // 新增：显示 "中" 或 "英"
        Exit,
    }
    