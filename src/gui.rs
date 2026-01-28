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
        setup_custom_fonts(&cc.egui_ctx);
        
        let mut visuals = egui::Visuals::dark();
        visuals.window_rounding = 6.0.into();
        visuals.window_shadow = egui::epaint::Shadow::small_dark();
        visuals.override_text_color = Some(egui::Color32::from_rgb(240, 240, 240));
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_black_alpha(180);
        cc.egui_ctx.set_visuals(visuals);
        
        Self {
            rx,
            pinyin: String::new(),
            candidates: Vec::new(),
            selected: 0,
        }
    }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // 尝试加载系统中的中文字体
    let font_paths = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
    ];

    let mut font_data = None;
    for path in font_paths {
        if let Ok(data) = std::fs::read(path) {
            font_data = Some(data);
            break;
        }
    }

    if let Some(data) = font_data {
        fonts.font_data.insert(
            "my_font".to_owned(),
            egui::FontData::from_owned(data),
        );

        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap()
            .insert(0, "my_font".to_owned());
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap()
            .push("my_font".to_owned());
        
        ctx.set_fonts(fonts);
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
        
        // 关键：在不需要时直接完全隐藏窗口
        frame.set_visible(is_visible);

        if is_visible {
            // 背景完全透明
            let panel_frame = egui::Frame::none()
                .fill(egui::Color32::TRANSPARENT);

            egui::CentralPanel::default()
                .frame(panel_frame)
                .show(ctx, |ui| {
                    // 容器：模拟类似搜狗/微信输入法的极简条状布局
                    egui::Frame::none()
                        .fill(egui::Color32::from_black_alpha(200))
                        .rounding(4.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80)))
                        .show(ui, |ui| {
                            // 允许拖动
                            if ui.interact(ui.max_rect(), ui.id(), egui::Sense::drag()).dragged() {
                                frame.drag_window();
                            }

                            ui.horizontal(|ui| {
                                // 拼音预览区
                                ui.label(egui::RichText::new(&self.pinyin)
                                    .color(egui::Color32::from_rgb(120, 180, 255))
                                    .size(17.0));
                                
                                if !self.candidates.is_empty() {
                                    ui.add_space(5.0);
                                    ui.separator();
                                    ui.add_space(5.0);

                                    let page_start = (self.selected / 5) * 5;
                                    let page_end = (page_start + 5).min(self.candidates.len());
                                    
                                    ui.spacing_mut().item_spacing.x = 12.0;
                                    for i in page_start..page_end {
                                        let cand = &self.candidates[i];
                                        let is_selected = i == self.selected;
                                        let display_idx = (i % 5) + 1;
                                        
                                        let text = format!("{}.{}", display_idx, cand);
                                        
                                        if is_selected {
                                            // 选中的词：高亮底色或金黄色文字
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_rgb(255, 255, 255))
                                                .background_color(egui::Color32::from_rgb(0, 102, 204))
                                                .size(18.0)
                                                .strong());
                                        } else {
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_gray(220))
                                                .size(17.0));
                                        }
                                    }
                                    
                                    if self.candidates.len() > 5 {
                                        ui.add_space(4.0);
                                        let current_page = (self.selected / 5) + 1;
                                        let total_pages = (self.candidates.len() + 4) / 5;
                                        ui.label(egui::RichText::new(format!("{}/{}", current_page, total_pages))
                                            .color(egui::Color32::from_gray(100))
                                            .size(13.0));
                                    }
                                }
                            });
                        });
                });
        }

        if updated || is_visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

pub fn start_gui(rx: Receiver<(String, Vec<String>, usize)>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(600.0, 45.0)),
        initial_window_pos: Some(egui::pos2(200.0, 200.0)),
        
        always_on_top: true,
        decorated: false,
        transparent: true,
        icon_data: None, // 不设置图标
        
        #[cfg(target_os = "linux")]
        follow_system_theme: true,
        
        ..Default::default()
    };

    let _ = eframe::run_native(
        "", // 标题设置为空，有助于在某些任务栏隐藏
        options,
        Box::new(|cc| Box::new(CandidateApp::new(cc, rx))),
    );
}