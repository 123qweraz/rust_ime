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
        visuals.window_rounding = 10.0.into();
        visuals.window_shadow = egui::epaint::Shadow::big_dark();
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
            egui::Area::new("candidate_area")
                .anchor(egui::Align2::LEFT_TOP, egui::vec2(120.0, 120.0))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_black_alpha(220))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::same(10.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&self.pinyin)
                                        .color(egui::Color32::from_rgb(100, 200, 255))
                                        .size(18.0)
                                        .strong());
                                });
                                
                                ui.add_space(8.0);
                                
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 15.0;
                                    for (i, cand) in self.candidates.iter().enumerate() {
                                        let is_selected = i == self.selected;
                                        let text = format!("{}.{}", i + 1, cand);
                                        
                                        if is_selected {
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_rgb(255, 215, 0))
                                                .size(18.0)
                                                .strong());
                                        } else {
                                            ui.label(egui::RichText::new(text)
                                                .size(17.0));
                                        }
                                    }
                                });
                            });
                        });
                });
        }

        // Only request repaint if we got new data or if we are visible (to keep UI responsive)
        if updated || is_visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(10));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

pub fn start_gui(rx: Receiver<(String, Vec<String>, usize)>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(400.0, 100.0)),
        always_on_top: true,
        decorated: false, // 无边框
        transparent: true, // 透明背景
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Rust IME Overlay",
        options,
        Box::new(|cc| Box::new(CandidateApp::new(cc, rx))),
    );
}