use eframe::egui;
use std::sync::mpsc::Receiver;

pub struct CandidateApp {
    rx: Receiver<(String, Vec<String>, usize)>,
    pinyin: String,
    candidates: Vec<String>,
    selected: usize,
    last_visible: bool,
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
            last_visible: false,
        }
    }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
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
        fonts.font_data.insert("my_font".to_owned(), egui::FontData::from_owned(data));
        fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap().insert(0, "my_font".to_owned());
        fonts.families.get_mut(&egui::FontFamily::Monospace).unwrap().push("my_font".to_owned());
        ctx.set_fonts(fonts);
    }
}

impl eframe::App for CandidateApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Ok((p, c, s)) = self.rx.try_recv() {
            self.pinyin = p;
            self.candidates = c;
            self.selected = s;
        }

        let is_visible = !self.pinyin.is_empty() || !self.candidates.is_empty();
        
        if is_visible != self.last_visible {
            frame.set_visible(is_visible);
            self.last_visible = is_visible;
        }

        if is_visible {
            // 核心修复：使用 monitor_size 获取屏幕分辨率进行定位
            let monitor_size = frame.info().window_info.monitor_size.unwrap_or(egui::vec2(1920.0, 1080.0));
            let window_size = egui::vec2(600.0, 50.0);
            
            // 计算右下角坐标 (留出边距)
            let target_pos = egui::pos2(
                monitor_size.x - window_size.x - 30.0,
                monitor_size.y - window_size.y - 70.0
            );
            
            frame.set_window_pos(target_pos);
            frame.set_always_on_top(true);

            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(egui::Color32::TRANSPARENT))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(egui::Color32::from_black_alpha(220))
                        .rounding(4.0)
                        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                        .stroke(egui::Stroke::new(1.2, egui::Color32::from_gray(90)))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(&self.pinyin)
                                    .color(egui::Color32::from_rgb(100, 200, 255))
                                    .size(18.0));
                                
                                if !self.candidates.is_empty() {
                                    ui.add_space(8.0);
                                    ui.separator();
                                    ui.add_space(8.0);

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
                                                .color(egui::Color32::WHITE)
                                                .background_color(egui::Color32::from_rgb(0, 102, 204))
                                                .size(20.0)
                                                .strong());
                                        } else {
                                            ui.label(egui::RichText::new(text)
                                                .color(egui::Color32::from_gray(220))
                                                .size(18.0));
                                        }
                                    }
                                }
                            });
                        });
                });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

pub fn start_gui(rx: Receiver<(String, Vec<String>, usize)>) {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(600.0, 55.0)),
        // 初始位置设在屏幕外，防止闪烁
        initial_window_pos: Some(egui::pos2(5000.0, 5000.0)),
        
        always_on_top: true,
        decorated: false,
        transparent: true,
        resizable: false,
        
        #[cfg(target_os = "linux")]
        follow_system_theme: false,
        
        ..Default::default()
    };

    let _ = eframe::run_native(
        "rust-ime-overlay",
        options,
        Box::new(|cc| Box::new(CandidateApp::new(cc, rx))),
    );
}
