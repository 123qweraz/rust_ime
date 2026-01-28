use eframe::egui;
use std::sync::mpsc::Receiver;

pub struct CandidateApp {
    rx: Receiver<(String, Vec<String>, usize)>,
    pinyin: String,
    candidates: Vec<String>,
    selected: usize,
}

impl CandidateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<(String, Vec<String>, usize)>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
        visuals.window_shadow = egui::epaint::Shadow::small_dark();
        visuals.override_text_color = Some(egui::Color32::from_rgb(220, 220, 220));
        cc.egui_ctx.set_visuals(visuals);
        
        Self {
            rx,
            pinyin: String::new(),
            candidates: Vec::new(),
            selected: 0,
        }
    }
}

impl eframe::App for CandidateApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut updated = false;
        while let Ok((p, c, s)) = self.rx.try_recv() {
            self.pinyin = p;
            self.candidates = c;
            self.selected = s;
            updated = true;
        }

        let is_visible = !self.pinyin.is_empty() || !self.candidates.is_empty();
        frame.set_visible(is_visible);

        if is_visible {
            // 设置窗口背景透明
            ctx.set_visuals(egui::Visuals {
                panel_fill: egui::Color32::TRANSPARENT,
                ..egui::Visuals::dark()
            });

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_black_alpha(210))
                        .rounding(6.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(70)))
                        .show(ui, |ui| {
                            // 使窗口可拖动
                            if ui.interact(ui.max_rect(), ui.id(), egui::Sense::drag()).dragged() {
                                frame.drag_window();
                            }

                            ui.horizontal(|ui| {
                                // 拼音部分
                                ui.label(egui::RichText::new(&self.pinyin)
                                    .color(egui::Color32::from_rgb(100, 200, 255))
                                    .size(18.0)
                                    .strong());
                                
                                if !self.candidates.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);
                                    
                                    // 候选词部分 (显示当前页，每页5个)
                                    let page_start = (self.selected / 5) * 5;
                                    let page_end = (page_start + 5).min(self.candidates.len());
                                    
                                    ui.spacing_mut().item_spacing.x = 15.0;
                                    for i in page_start..page_end {
                                        let cand = &self.candidates[i];
                                        let is_selected = i == self.selected;
                                        let display_idx = (i % 5) + 1;
                                        let text = format!("{}.{}", display_idx, cand);
                                        
                                        if is_selected {
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_rgb(255, 215, 0))
                                                .size(19.0)
                                                .strong());
                                        } else {
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_gray(210))
                                                .size(18.0));
                                        }
                                    }

                                    if self.candidates.len() > 5 {
                                        ui.add_space(5.0);
                                        let total_pages = (self.candidates.len() + 4) / 5;
                                        let current_page = (self.selected / 5) + 1;
                                        ui.label(egui::RichText::new(format!("{}/{}", current_page, total_pages))
                                            .color(egui::Color32::from_gray(120))
                                            .size(14.0));
                                    }
                                }
                            });
                        });
                });
        }

        // 保持高刷新率以保证响应速度，尤其是在输入时
        if updated || is_visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(10));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

pub fn start_gui(rx: Receiver<(String, Vec<String>, usize)>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 60.0)),
        initial_window_pos: Some(egui::pos2(100.0, 100.0)), // 设置一个默认位置，避免出现在屏幕中央遮挡
        always_on_top: true,
        decorated: false, 
        transparent: true,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Rust IME Overlay",
        options,
        Box::new(|cc| Box::new(CandidateApp::new(cc, rx))),
    );
}
