use eframe::egui;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

pub struct CandidateApp {
    rx: Receiver<(String, Vec<String>, usize)>,
    pinyin: String,
    candidates: Vec<String>,
    selected: usize,
}

impl CandidateApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rx: Receiver<(String, Vec<String>, usize)>) -> Self {
        // 自定义样式，使其看起来像一个输入法
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 8.0.into();
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
        // 实时接收来自内核的消息
        while let Ok((p, c, s)) = self.rx.try_recv() {
            self.pinyin = p;
            self.candidates = c;
            self.selected = s;
        }

        // 如果没有输入，隐藏窗口
        if self.pinyin.is_empty() && self.candidates.is_empty() {
            frame.set_visible(false);
        } else {
            frame.set_visible(true);
        }

        // 渲染选词框
        egui::Area::new("candidate_area")
            .anchor(egui::Align2::LEFT_TOP, egui::vec2(100.0, 100.0)) // 暂时固定位置，后续可优化为跟随光标
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .fill(egui::Color32::from_black_alpha(200))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&self.pinyin).color(egui::Color32::LIGHT_BLUE).strong());
                            });
                            
                            ui.add_space(4.0);
                            
                            ui.horizontal(|ui| {
                                for (i, cand) in self.candidates.iter().enumerate() {
                                    let text = format!("{}.{}", i + 1, cand);
                                    if i == self.selected {
                                        ui.label(egui::RichText::new(text).color(egui::Color32::YELLOW).strong().underline());
                                    } else {
                                        ui.label(text);
                                    }
                                }
                            });
                        });
                    });
            });

        // 强制每秒刷新几次，确保及时响应消息
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
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