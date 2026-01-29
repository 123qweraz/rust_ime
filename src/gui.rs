use gtk4::prelude::*;
use gtk4::{Window, Label, Box, Orientation, CssProvider};
use gdk4::Display;
use gtk4_layer_shell::{LayerShell, Layer, Edge, KeyboardMode};
use std::sync::mpsc::Receiver;
use glib::MainContext;
use crate::config::Config;

#[derive(Debug)]
pub enum GuiEvent {
    Update {
        pinyin: String,
        candidates: Vec<String>,
        hints: Vec<String>,
        selected: usize,
    },
    Keystroke(String),
    ShowLearning(String, String), // 汉字, 提示
    ClearKeystrokes,
    ApplyConfig(Config),
    #[allow(dead_code)]
    Exit,
}

pub fn start_gui(rx: Receiver<GuiEvent>, initial_config: Config) {
    if gtk4::init().is_err() {
        eprintln!("[GUI] Failed to initialize GTK4.");
        return;
    }

    let is_layer_supported = gtk4_layer_shell::is_supported();

    // --- 窗口创建 ---
    let window = Window::builder().title("Rust IME Candidates").decorated(false).can_focus(false).focusable(false).resizable(false).build();
    let key_window = Window::builder().title("Keystroke Display").decorated(false).can_focus(false).focusable(false).resizable(false).build();
    
    if is_layer_supported {
        window.init_layer_shell();
        window.set_namespace("rust-ime-candidates");
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_exclusive_zone(0);

        key_window.init_layer_shell();
        key_window.set_namespace("rust-ime-keystrokes");
        key_window.set_layer(Layer::Overlay);
        key_window.set_keyboard_mode(KeyboardMode::None);
        key_window.set_exclusive_zone(0);
    }

    window.add_css_class("ime-window");
    key_window.add_css_class("keystroke-window");

    let main_box = Box::new(Orientation::Horizontal, 8);
    main_box.set_widget_name("main-container");
    window.set_child(Some(&main_box));

    let pinyin_label = Label::new(None);
    pinyin_label.set_widget_name("pinyin-label");
    main_box.append(&pinyin_label);

    let candidates_box = Box::new(Orientation::Horizontal, 12);
    candidates_box.set_widget_name("candidates-box");
    main_box.append(&candidates_box);

    let key_box = Box::new(Orientation::Horizontal, 6);
    key_box.set_widget_name("keystroke-container");
    key_window.set_child(Some(&key_box));

    let css_provider = CssProvider::new();
    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(&display, &css_provider, gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    // --- 配置应用逻辑 ---
    let apply_style = move |conf: &Config, css: &CssProvider, w: &Window, kw: &Window| {
        let app = &conf.appearance;
        
        // 动态生成对齐苹果审美的 CSS
        let css_data = format!(r#"
            window.ime-window, window.keystroke-window {{ background-color: transparent; }}
            
            #main-container {{
                background-color: {cand_bg};
                border: 1px solid rgba(255, 255, 255, 0.1);
                border-radius: 14px;
                padding: 8px 14px;
                box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
            }}

            #keystroke-container {{
                background-color: {key_bg};
                border: 1px solid rgba(255, 255, 255, 0.1);
                border-radius: 14px;
                padding: 8px 14px;
                box-shadow: 0 8px 32px rgba(0, 0, 0, 0.35);
            }}

            #pinyin-label {{
                color: #0071e3;
                font-size: {cand_font}pt;
                font-weight: 700;
                margin-right: 6px;
                padding-right: 12px;
                border-right: 1px solid rgba(255, 255, 255, 0.1);
            }}

            .candidate-item {{
                padding: 4px 10px;
                border-radius: 8px;
                margin: 0 2px;
                transition: all 0.2s cubic-bezier(0.25, 0.1, 0.25, 1);
            }}

            .candidate-selected {{
                background-color: #0071e3;
                box-shadow: 0 4px 12px rgba(0, 113, 227, 0.3);
            }}

            .candidate-text {{ 
                color: #ffffff; 
                font-size: {cand_font}pt; 
                font-weight: 500; 
                letter-spacing: 0.2px;
            }}

            .index {{
                font-size: 10pt;
                font-weight: 600;
                color: rgba(255, 255, 255, 0.45);
                margin-right: 8px;
            }}

            .hint-text {{
                color: rgba(255, 255, 255, 0.5);
                font-size: 10pt;
                margin-left: 6px;
            }}

            .candidate-selected .index, .candidate-selected .hint-text {{
                color: rgba(255, 255, 255, 0.8);
            }}

            /* 按键回显风格：物理质感键帽 */
            .key-label {{
                background: linear-gradient(to bottom, #4a4a4a, #2c2c2c);
                color: #f5f5f7;
                font-family: 'SF Pro Text', 'Sans', sans-serif;
                font-size: {key_font}pt;
                font-weight: 600;
                padding: 6px 14px;
                border-radius: 8px;
                border: 1px solid #1a1a1a;
                box-shadow: inset 0 1px 0 rgba(255,255,255,0.05), 0 4px 8px rgba(0,0,0,0.3);
                margin: 3px;
            }}
        "#, 
        cand_bg = app.candidate_bg_color,
        key_bg = app.keystroke_bg_color,
        cand_font = app.candidate_font_size,
        key_font = app.keystroke_font_size);
        
        css.load_from_data(&css_data);

        // 应用位置 (LayerShell)
        if gtk4_layer_shell::is_supported() {
            // 候选词位置
            match app.candidate_anchor.as_str() {
                "top" => { w.set_anchor(Edge::Bottom, false); w.set_anchor(Edge::Top, true); }
                _ => { w.set_anchor(Edge::Top, false); w.set_anchor(Edge::Bottom, true); }
            }
            w.set_margin(Edge::Bottom, app.candidate_margin_y);
            w.set_margin(Edge::Top, app.candidate_margin_y);
            w.set_margin(Edge::Left, app.candidate_margin_x);

            // 按键回显位置
            kw.set_anchor(Edge::Bottom, false); kw.set_anchor(Edge::Top, false);
            kw.set_anchor(Edge::Left, false); kw.set_anchor(Edge::Right, false);
            match app.keystroke_anchor.as_str() {
                "top_right" => { kw.set_anchor(Edge::Top, true); kw.set_anchor(Edge::Right, true); }
                "top_left" => { kw.set_anchor(Edge::Top, true); kw.set_anchor(Edge::Left, true); }
                "bottom_left" => { kw.set_anchor(Edge::Bottom, true); kw.set_anchor(Edge::Left, true); }
                _ => { kw.set_anchor(Edge::Bottom, true); kw.set_anchor(Edge::Right, true); }
            }
            kw.set_margin(Edge::Bottom, app.keystroke_margin_y);
            kw.set_margin(Edge::Top, app.keystroke_margin_y);
            kw.set_margin(Edge::Left, app.keystroke_margin_x);
            kw.set_margin(Edge::Right, app.keystroke_margin_x);
        }
    };

    // 初始应用配置
    apply_style(&initial_config, &css_provider, &window, &key_window);

    let (tx, gtk_rx) = MainContext::channel::<GuiEvent>(glib::Priority::default());
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let is_exit = matches!(msg, GuiEvent::Exit);
            if tx.send(msg).is_err() || is_exit { break; }
        }
    });

    let window_c = window.clone();
    let key_window_c = key_window.clone();
    let pinyin_label_c = pinyin_label.clone();
    let candidates_box_c = candidates_box.clone();
    let key_box_c = key_box.clone();
    let css_p_c = css_provider.clone();
    let mut current_config = initial_config;

    gtk_rx.attach(None, move |event| {
        match event {
            GuiEvent::ApplyConfig(conf) => {
                apply_style(&conf, &css_p_c, &window_c, &key_window_c);
                current_config = conf;
            }
            GuiEvent::Update { pinyin, candidates, hints, selected } => {
                if pinyin.is_empty() && candidates.is_empty() {
                    window_c.set_opacity(0.0);
                    while let Some(child) = candidates_box_c.first_child() { candidates_box_c.remove(&child); }
                    pinyin_label_c.set_text("");
                    return glib::Continue(true);
                }
                window_c.set_opacity(1.0);
                pinyin_label_c.set_text(&pinyin);
                while let Some(child) = candidates_box_c.first_child() { candidates_box_c.remove(&child); }
                let page_start = (selected / 5) * 5;
                let page_end = (page_start + 5).min(candidates.len());
                for i in page_start..page_end {
                    let cand_box = Box::new(Orientation::Horizontal, 0);
                    cand_box.add_css_class("candidate-item");
                    let idx_label = Label::new(Some(&format!("{}", (i % 5) + 1)));
                    idx_label.add_css_class("index");
                    let val_label = Label::new(Some(&candidates[i]));
                    val_label.add_css_class("candidate-text");
                    cand_box.append(&idx_label);
                    cand_box.append(&val_label);
                    if let Some(hint) = hints.get(i) {
                        if !hint.is_empty() {
                            let hint_label = Label::new(Some(&hint));
                            hint_label.add_css_class("hint-text");
                            cand_box.append(&hint_label);
                        }
                    }
                    if i == selected { cand_box.add_css_class("candidate-selected"); }
                    candidates_box_c.append(&cand_box);
                }
            },
            GuiEvent::Keystroke(key_name) => {
                let label = Label::new(Some(&key_name));
                label.add_css_class("key-label");
                key_box_c.append(&label);
                key_window_c.set_opacity(1.0);
                
                let kb_weak = key_box_c.downgrade();
                let label_weak = label.downgrade();
                let kw_weak = key_window_c.downgrade();
                let timeout = current_config.appearance.keystroke_timeout_ms;
                
                glib::timeout_add_local(std::time::Duration::from_millis(timeout), move || {
                    let kb: Box = if let Some(kb) = kb_weak.upgrade() { kb } else { return glib::Continue(false); };
                    let l = if let Some(l) = label_weak.upgrade() { l } else { return glib::Continue(false); };
                    
                    kb.remove(&l);
                    if kb.first_child().is_none() {
                        if let Some(kw) = kw_weak.upgrade() { kw.set_opacity(0.0); }
                    }
                    glib::Continue(false)
                });
            },
            GuiEvent::ShowLearning(hanzi, hint) => {
                // 清空旧内容
                while let Some(child) = key_box_c.first_child() { key_box_c.remove(&child); }
                
                let text = if hint.is_empty() { hanzi } else { format!("{} {}", hanzi, hint) };
                let label = Label::new(Some(&text));
                label.add_css_class("key-label");
                label.set_margin_start(4);
                label.set_margin_end(4);
                
                key_box_c.append(&label);
                key_window_c.set_opacity(1.0);
            },
            GuiEvent::ClearKeystrokes => {
                while let Some(child) = key_box_c.first_child() { key_box_c.remove(&child); }
                key_window_c.set_opacity(0.0);
            },
            GuiEvent::Exit => { window_c.close(); key_window_c.close(); return glib::Continue(false); }
        }
        glib::Continue(true)
    });

    window.set_opacity(0.0);
    window.present();
    key_window.set_opacity(0.0);
    key_window.present();
    let loop_ = glib::MainLoop::new(None, false);
    loop_.run();
}