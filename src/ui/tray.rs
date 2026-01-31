use ksni::menu::{StandardItem, MenuItem};
use ksni::{Tray, ToolTip, TrayService, Handle};
use std::sync::mpsc::Sender;
use tiny_skia::*;

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
    ToggleLearning,
    ReloadConfig,
    CyclePreview,
}

pub struct ImeTray {
    pub chinese_enabled: bool,
    pub active_profile: String,
    pub show_candidates: bool,
    pub show_notifications: bool,
    pub show_keystrokes: bool,
    pub learning_mode: bool,
    pub preview_mode: String,
    pub tx: Sender<TrayEvent>,
}

impl Tray for ImeTray {
    fn icon_name(&self) -> String {
        "rust-ime-dynamic".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let size = 22;
        let mut pixmap = Pixmap::new(size, size).unwrap();
        
        let mut paint = Paint::default();
        if self.chinese_enabled {
            paint.set_color_rgba8(247, 76, 0, 255); // Rust Orange
        } else {
            paint.set_color_rgba8(74, 74, 74, 255); // Dark Grey
        }
        paint.anti_alias = true;

        // 绘制圆角背景
        let path = {
            let mut pb = PathBuilder::new();
            pb.move_to(5.0, 2.0);
            pb.line_to(17.0, 2.0);
            pb.quad_to(20.0, 2.0, 20.0, 5.0);
            pb.line_to(20.0, 17.0);
            pb.quad_to(20.0, 20.0, 17.0, 20.0);
            pb.line_to(5.0, 20.0);
            pb.quad_to(2.0, 20.0, 2.0, 17.0);
            pb.line_to(2.0, 5.0);
            pb.quad_to(2.0, 2.0, 5.0, 2.0);
            pb.finish().unwrap()
        };
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);

        // 绘制一个简单的指示器 (白色的点)
        if self.chinese_enabled {
            let mut dot_paint = Paint::default();
            dot_paint.set_color_rgba8(255, 255, 255, 255);
            let center_rect = Rect::from_xywh(8.0, 8.0, 6.0, 6.0).unwrap();
            pixmap.fill_rect(center_rect, &dot_paint, Transform::identity(), None);
        }

        let rgba = pixmap.data().to_vec();
        // ARGB 转换 (ksni/DBus 预期是 ARGB，每像素 4 字节)
        let mut argb_data = Vec::with_capacity(rgba.len());
        for chunk in rgba.chunks_exact(4) {
            argb_data.push(chunk[3]); // A
            argb_data.push(chunk[0]); // R
            argb_data.push(chunk[1]); // G
            argb_data.push(chunk[2]); // B
        }

        vec![ksni::Icon {
            width: size as i32,
            height: size as i32,
            data: argb_data,
        }]
    }

    fn title(&self) -> String {
        format!("rust-IME ({})", if self.chinese_enabled { "中" } else { "英" })
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "rust-IME".to_string(),
            description: format!("Profile: {}\nGUI: {}\nPreview: {}\nLearning: {}", 
                self.active_profile,
                if self.show_candidates { "开" } else { "关" },
                self.preview_mode,
                if self.learning_mode { "开" } else { "关" }
            ),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: format!("模式: {}", if self.chinese_enabled { "中文" } else { "英文" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ToggleIme); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("词库: {}", self.active_profile),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::NextProfile); }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: format!("候选窗: {}", if self.show_candidates { "显示" } else { "隐藏" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ToggleGui); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("拼音预览: {}", if self.preview_mode == "pinyin" { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::CyclePreview); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("桌面通知: {}", if self.show_notifications { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ToggleNotify); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("按键回显: {}", if self.show_keystrokes { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ToggleKeystroke); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: format!("学习模式: {}", if self.learning_mode { "开启" } else { "关闭" }),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ToggleLearning); }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "配置中心 (Web)".to_string(),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::OpenConfig); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "重新加载配置".to_string(),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::ReloadConfig); }),
                ..Default::default()
            }.into(),
            StandardItem {
                label: "重启服务".to_string(),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::Restart); }),
                ..Default::default()
            }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "退出程序".to_string(),
                activate: Box::new(|this: &mut Self| { let _ = this.tx.send(TrayEvent::Exit); }),
                ..Default::default()
            }.into(),
        ]
    }
}

pub fn start_tray(
    chinese_enabled: bool, active_profile: String, show_candidates: bool,
    show_notifications: bool, show_keystrokes: bool, learning_mode: bool,
    preview_mode: String,
    event_tx: Sender<TrayEvent>
) -> Handle<ImeTray> {
    let service = ImeTray { chinese_enabled, active_profile, show_candidates, show_notifications, show_keystrokes, learning_mode, preview_mode, tx: event_tx };
    let tray_service = TrayService::new(service);
    let handle = tray_service.handle();
    std::thread::spawn(move || { let _ = tray_service.run(); });
    handle
}