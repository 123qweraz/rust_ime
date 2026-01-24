use ksni::menu::{StandardItem, MenuItem};
use ksni::{Tray, ToolTip, TrayService, Handle};
use std::sync::mpsc::Sender;

#[derive(Debug, Clone)]
pub enum TrayEvent {
    ToggleIme,
    NextProfile,
    Exit,
}

pub struct ImeTray {
    pub chinese_enabled: bool,
    pub active_profile: String,
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
            description: format!("Profile: {}", self.active_profile),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: format!("Mode: {}", if self.chinese_enabled { "Chinese" } else { "English" }),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::ToggleIme);
                }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("Profile: {}", self.active_profile),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayEvent::NextProfile);
                }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Exit".to_string(),
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
    event_tx: Sender<TrayEvent>
) -> Handle<ImeTray> {
    let service = ImeTray {
        chinese_enabled,
        active_profile,
        tx: event_tx,
    };
    let tray_service = TrayService::new(service);
    let handle = tray_service.handle();
    tray_service.spawn();
    handle
}