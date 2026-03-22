use crate::CandidateDisplay;
use rust_ime_core::Config;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

slint::include_modules!();

pub struct SlintDisplay {
    window: CandidateWindow,
    status_bar: StatusBar,
    config: Config,
}

impl SlintDisplay {
    pub fn new(config: Config) -> Self {
        let window = CandidateWindow::new().expect("Failed to create CandidateWindow");
        let status_bar = StatusBar::new().expect("Failed to create StatusBar");

        let mut display = Self {
            window,
            status_bar,
            config: config.clone(),
        };

        display.apply_style(&config);
        display
    }

    fn apply_style(&mut self, config: &Config) {
        let parse_color = |s: &str| -> slint::Color {
            if s.starts_with('#') && s.len() == 7 {
                let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(255);
                let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(255);
                let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(255);
                slint::Color::from_rgb_u8(r, g, b)
            } else {
                slint::Color::from_rgb_u8(9, 105, 218)
            }
        };

        self.window
            .set_show_english_aux(config.appearance.show_english_aux);
        self.window
            .set_show_stroke_aux(config.appearance.show_stroke_aux);
        self.window
            .set_show_translation(config.appearance.show_english_translation);
        self.window
            .set_is_horizontal(config.appearance.candidate_layout == "horizontal");

        self.window
            .set_bg_color(parse_color(&config.appearance.window_bg_color));
        self.window
            .set_accent_color(parse_color(&config.appearance.window_highlight_color));
        self.window
            .set_border_color(parse_color(&config.appearance.window_border_color));
        self.window
            .set_text_color(parse_color(&config.appearance.candidate_text.color));
        self.window
            .set_highlight_text_color(parse_color(&config.appearance.window_bg_color));

        let font_stack = format!(
            "{}, Segoe UI Emoji, Microsoft YaHei, Arial, system-ui",
            config.appearance.candidate_text.font_family
        );
        self.window
            .set_pinyin_font_family(SharedString::from(&font_stack));
        self.window
            .set_candidate_font_family(SharedString::from(&font_stack));

        self.window
            .set_pinyin_font_size(config.appearance.pinyin_text.font_size as f32);
        self.window
            .set_pinyin_font_weight(config.appearance.pinyin_text.font_weight as i32);
        self.window
            .set_candidate_font_size(config.appearance.candidate_text.font_size as f32);
        self.window
            .set_candidate_font_weight(config.appearance.candidate_text.font_weight as i32);
    }
}

impl CandidateDisplay for SlintDisplay {
    fn update_candidates(
        &mut self,
        pinyin: &str,
        candidates: Vec<crate::DisplayCandidate>,
        selected: usize,
    ) {
        if pinyin.is_empty() || !self.config.appearance.show_candidates {
            if self.window.window().is_visible() {
                self.window.set_is_visible(false);
                let _ = self.window.window().hide();
            }
            return;
        }

        self.window.set_pinyin(SharedString::from(pinyin));
        self.window.set_selected_index(selected as i32);

        let mut cand_models = Vec::new();
        for c in candidates {
            cand_models.push(CandidateData {
                text: SharedString::from(c.text),
                english_aux: SharedString::from(c.hint),
                stroke_aux: SharedString::from(""), // 暂不独立显示笔画，统一在 hint 中
            });
        }
        self.window
            .set_candidates(ModelRc::from(std::rc::Rc::new(VecModel::from(cand_models))));

        if !self.window.window().is_visible() {
            self.window.set_is_visible(true);
            let _ = self.window.window().show();
        }
    }

    fn update_status(&mut self, text: &str, chinese_enabled: bool) {
        if !text.is_empty() {
            self.status_bar.set_status_text(SharedString::from(text));
        }
        self.status_bar.set_chinese_enabled(chinese_enabled);

        if self.config.appearance.show_status_bar {
            if !self.status_bar.window().is_visible() {
                let _ = self.status_bar.window().show();
            }
        } else if self.status_bar.window().is_visible() {
            let _ = self.status_bar.window().hide();
        }
    }

    fn move_to(&mut self, x: i32, y: i32) {
        self.window
            .window()
            .set_position(slint::WindowPosition::Physical(
                slint::PhysicalPosition::new(x, y),
            ));
    }

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.window.set_is_visible(false);
            let _ = self.window.window().hide();
        }
    }

    fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
        self.apply_style(config);
    }

    fn close(&mut self) {
        let _ = self.window.window().hide();
        let _ = self.status_bar.window().hide();
    }
}
