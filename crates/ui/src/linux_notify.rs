use crate::{CandidateDisplay, DisplayCandidate};
use notify_rust::{Hint, Notification, NotificationHandle};
use rust_ime_core::Config;

pub struct LinuxNotifyDisplay {
    active_notification: Option<NotificationHandle>,
    config: Config,
    last_content: String, // 缓存内容，避免重复发送完全相同的内容
}

impl LinuxNotifyDisplay {
    pub fn new(config: Config) -> Self {
        Self {
            active_notification: None,
            config,
            last_content: String::new(),
        }
    }
}

impl CandidateDisplay for LinuxNotifyDisplay {
    fn update_candidates(
        &mut self,
        pinyin: &str,
        candidates: Vec<DisplayCandidate>,
        selected: usize,
    ) {
        if !self.config.linux.enable_notification_candidates {
            if let Some(h) = self.active_notification.take() {
                h.close();
            }
            return;
        }

        if pinyin.is_empty() {
            if let Some(h) = self.active_notification.take() {
                h.close();
            }
            self.last_content.clear();
            return;
        }

        let mut notify_body = String::new();
        for (i, c) in candidates.iter().enumerate() {
            if i == selected {
                notify_body.push_str(&format!("【{}】 ", c.full_display));
            } else {
                notify_body.push_str(&format!("{} ", c.full_display));
            }
        }

        let current_content = format!("{}:{}", pinyin, notify_body);
        if current_content == self.last_content {
            return;
        }
        self.last_content = current_content;

        if let Some(ref mut h) = self.active_notification {
            h.summary(pinyin);
            h.body(&notify_body);
            // 每次更新都显式设置 transient 确保不存入通知历史堆栈
            h.hint(Hint::Transient(true));
            h.hint(Hint::Custom(
                "x-canonical-private-synchronous".to_string(),
                "true".to_string(),
            ));
            h.update();
        } else {
            self.active_notification = Notification::new()
                .summary(pinyin)
                .body(&notify_body)
                .appname("rust-ime")
                .hint(Hint::Transient(true))
                .hint(Hint::Custom(
                    "x-canonical-private-synchronous".to_string(),
                    "true".to_string(),
                ))
                .timeout(0)
                .show()
                .ok();
        }
    }

    fn update_status(&mut self, text: &str, _chinese_enabled: bool) {
        if text.is_empty() {
            return;
        }
        let _ = Notification::new()
            .summary("Rust IME")
            .body(text)
            .appname("rust-ime")
            .hint(Hint::Transient(true))
            .timeout(1500)
            .show();
    }

    fn move_to(&mut self, _x: i32, _y: i32) {}

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            if let Some(h) = self.active_notification.take() {
                h.close();
            }
            self.last_content.clear();
        }
    }

    fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
    }

    fn close(&mut self) {
        if let Some(h) = self.active_notification.take() {
            h.close();
        }
    }
}
