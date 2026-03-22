pub mod tray;
pub mod web;
pub mod gui_slint;
pub mod linux_notify;
pub mod slint_window;
pub mod platform;
pub use gui_slint as gui;

use rust_ime_core::Config;

/// 预格式化的候选词信息，UI 应当直接显示这些字符串而不再做逻辑拼接
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayCandidate {
    pub text: String,         // 候选词文字 (如: "你好")
    pub label: String,        // 序号标签 (如: "1.")
    pub hint: String,         // 辅助提示 (如: "nh")
    pub full_display: String, // 完整显示文本 (如: "1.你好(nh)")
}

/// 核心显示接口：解耦 Slint 窗口与 Linux 桌面通知
pub trait CandidateDisplay {
    /// 更新候选词列表及拼音
    fn update_candidates(&mut self, pinyin: &str, candidates: Vec<DisplayCandidate>, selected: usize);
    
    /// 更新状态栏显示（中/英模式文字）
    fn update_status(&mut self, text: &str, chinese_enabled: bool);
    
    /// 移动显示位置（通常仅对窗口 UI 有效）
    fn move_to(&mut self, x: i32, y: i32);
    
    /// 设置全局显隐状态
    fn set_visible(&mut self, visible: bool);
    
    /// 应用配置更新
    fn apply_config(&mut self, config: &Config);

    /// 销毁或关闭显示
    fn close(&mut self);
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppState {
    pub chinese_enabled: bool,
    pub active_profile: String,
    pub show_status_bar_pref: bool,
    pub show_candidates_pref: bool,
    pub is_ime_active: bool, // 窗口是否获得焦点/输入法是否激活
    pub pinyin: String,
    pub candidates: Vec<DisplayCandidate>,
    pub selected_index: usize,
    pub status_text: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum GuiEvent {
    SyncState(AppState), // 单一数据源同步
    ForceStatusVisible(bool), // 强制、独立的状态栏显隐控制 (不受任何焦点影响)
    Update {
        pinyin: String,
        candidates: Vec<DisplayCandidate>,
        selected: usize,
        sentence: String,
        cursor_pos: usize,
        commit_mode: String,
    },
    MoveTo { x: i32, y: i32 },
    ApplyConfig(Box<Config>),
    ShowStatus(String, bool), // 状态文字, 是否为中文模式 (用于更新文字)
    UpdateStatusBarVisible(bool), // 手动更新状态栏显隐
    SetVisible(bool),         // 窗口显隐 (用于输入法激活/停用)
    OpenTrayMenu { x: i32, y: i32, chinese_enabled: bool, active_profile: String },
    Exit,
}
