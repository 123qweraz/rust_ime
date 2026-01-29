use ksni::menu::{StandardItem, MenuItem};
use ksni::{Tray, ToolTip, TrayService, Handle};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ToggleIme,
    NextProfile,
    OpenConfig,
    Restart,
    Exit,
    ToggleGui,
    ToggleNotify,
    ToggleKeystroke,
}

pub struct ImeTray {
    pub chinese_enabled: bool,
    pub active_profile: String,
    pub show_candidates: bool,
    pub show_notifications: bool,
    pub show_keystrokes: bool,
    pub tx: Sender<TrayEvent>,
}

impl Tray for ImeTray {
    fn icon_name(&self) -> String {
        if self.chinese_enabled {
            "input-keyboard".to_string() 
        } else {
            "keyboard".to_string()
        }
    }

    fn title(&self) -> String {
        format!("Blind IME ({})", if self.chinese_enabled { "中" } else { "英" })
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Blind IME".to_string(),
            description: format!("Profile: {}\nGUI: {}\nNotify: {}\nKeystroke: {}", 
                self.active_profile,
                if self.show_candidates { "开" } else { "关" },
                if self.show_notifications { "开" } else { "关" },
                if self.show_keystrokes { "开" } else { "关" }
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: format!("模式: {}", if self.chinese_enabled { "中文" } else { "英文" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleIme);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("词库: {}", self.active_profile),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::NextProfile);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: format!("候选窗: {}", if self.show_candidates { "显示" } else { "隐藏" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleGui);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("桌面通知: {}", if self.show_notifications { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleNotify);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("按键回显: {}", if self.show_keystrokes { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleKeystroke);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "配置中心 (Web)".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::OpenConfig);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "重启服务".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::Restart);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "退出程序".to_string(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::Exit);
                }),
                ..Default::default()
            }.into(),
        ]
    }
}

pub fn start_tray(
    chinese_enabled: bool, 
    active_profile: String, 
    show_candidates: bool,
    show_notifications: bool,
    show_keystrokes: bool,
    event_tx: Sender<TrayEvent>
) -> Handle<ImeTray> {
    let service = ImeTray {
        chinese_enabled,
        active_profile,
        show_candidates,
        show_notifications,
        show_keystrokes,
        tx: event_tx,
    };
    let tray_service = TrayService::new(service);
    let handle = tray_service.handle();
    
    // Use an explicit thread to ensure the tray service runs independently
    std::thread::spawn(move || {
        let _ = tray_service.run();
    });
    
    handle
}