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
        // 动态变更名称，强制部分桌面环境（如 GNOME）刷新像素缓存
        if self.chinese_enabled { "rust-ime-zh".into() } else { "rust-ime-en".into() }
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let size = 22;
        let mut pixmap = Pixmap::new(size, size).unwrap();
        
        let mut paint = Paint::default();
        if self.chinese_enabled {
            paint.set_color_rgba8(255, 128, 0, 255); // 标准橙色
        } else {
            paint.set_color_rgba8(60, 60, 60, 255); // 深灰色
        }
        paint.anti_alias = true;

        // 1. 绘制圆角背景
        let bg_path = {
            let mut pb = PathBuilder::new();
            let r = 4.0;
            let rect = Rect::from_xywh(2.0, 2.0, 18.0, 18.0).unwrap();
            pb.move_to(rect.left() + r, rect.top());
            pb.line_to(rect.right() - r, rect.top());
            pb.quad_to(rect.right(), rect.top(), rect.right(), rect.top() + r);
            pb.line_to(rect.right(), rect.bottom() - r);
            pb.quad_to(rect.right(), rect.bottom(), rect.right() - r, rect.bottom());
            pb.line_to(rect.left() + r, rect.bottom());
            pb.quad_to(rect.left(), rect.bottom(), rect.left(), rect.bottom() - r);
            pb.line_to(rect.left(), rect.top() + r);
            pb.quad_to(rect.left(), rect.top(), rect.left() + r, rect.top());
            pb.finish().unwrap()
        };
        pixmap.fill_path(&bg_path, &paint, FillRule::Winding, Transform::identity(), None);

        // 2. 绘制内容
        let mut icon_paint = Paint::default();
        icon_paint.set_color_rgba8(255, 255, 255, 255);
        icon_paint.anti_alias = true;

        if self.chinese_enabled {
            // 用 5 个矩形拼出一个结实的“中”字
            let p = &icon_paint;
            // 矩形的上下左右边
            pixmap.fill_rect(Rect::from_xywh(6.0, 8.5, 10.0, 1.5).unwrap(), p, Transform::identity(), None);   // 上
            pixmap.fill_rect(Rect::from_xywh(6.0, 13.0, 10.0, 1.5).unwrap(), p, Transform::identity(), None);  // 下
            pixmap.fill_rect(Rect::from_xywh(6.0, 8.5, 1.5, 6.0).unwrap(), p, Transform::identity(), None);    // 左
            pixmap.fill_rect(Rect::from_xywh(14.5, 8.5, 1.5, 6.0).unwrap(), p, Transform::identity(), None);  // 右
            // 中间那一竖
            pixmap.fill_rect(Rect::from_xywh(10.25, 5.0, 1.5, 12.0).unwrap(), p, Transform::identity(), None);
        } else {
            // 键盘网格 (3x2)
            for y in 0..2 {
                for x in 0..3 {
                    let k_rect = Rect::from_xywh(6.0 + x as f32 * 4.0, 9.0 + y as f32 * 4.0, 2.5, 2.5).unwrap();
                    pixmap.fill_rect(k_rect, &icon_paint, Transform::identity(), None);
                }
            }
        }

        let rgba = pixmap.data().to_vec();
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