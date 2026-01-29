use gtk4::prelude::*;
use gtk4::{Window, Label, Box, Orientation, CssProvider};
use gdk4::Display;
use gtk4_layer_shell::{LayerShell, Layer, Edge, KeyboardMode};
use std::sync::mpsc::Receiver;
use glib::MainContext;

#[derive(Debug)]
pub enum GuiEvent {
    Update {
        pinyin: String,
        candidates: Vec<String>,
        hints: Vec<String>,
        selected: usize,
    },
    Keystroke(String),
    ClearKeystrokes,
    Exit,
}

pub fn start_gui(rx: Receiver<GuiEvent>) {
    // 基础初始化
    if gtk4::init().is_err() {
        eprintln!("Failed to initialize GTK4.");
        return;
    }

    let is_layer_supported = gtk4_layer_shell::is_supported();
    if !is_layer_supported {
        eprintln!("[GUI] Warning: Layer shell not supported. Falling back to normal windows.");
    }

    // --- Candidate Window ---
    let window = Window::builder()
        .title("Rust IME Candidates")
        .decorated(false)
        .can_focus(false)
        .focusable(false)
        .resizable(false)
        // 给一个初始尺寸，防止某些合成器在 0x0 时崩溃
        .default_width(200)
        .default_height(40)
        .build();
    
    if is_layer_supported {
        // 必须最先初始化 LayerShell
        window.init_layer_shell();
        window.set_namespace("rust-ime-candidates");
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 120);
    }
    window.add_css_class("ime-window");
    
    let main_box = Box::new(Orientation::Horizontal, 8);
    main_box.set_widget_name("main-container");
    window.set_child(Some(&main_box));

    let pinyin_label = Label::new(None);
    pinyin_label.set_widget_name("pinyin-label");
    main_box.append(&pinyin_label);

    let candidates_box = Box::new(Orientation::Horizontal, 12);
    candidates_box.set_widget_name("candidates-box");
    main_box.append(&candidates_box);

    // --- Keystroke Window ---
    let key_window = Window::builder()
        .title("Keystroke Display")
        .decorated(false)
        .can_focus(false)
        .focusable(false)
        .resizable(false)
        .default_width(100)
        .default_height(40)
        .build();
    
    if is_layer_supported {
        key_window.init_layer_shell();
        key_window.set_namespace("rust-ime-keystrokes");
        key_window.set_layer(Layer::Overlay);
        key_window.set_keyboard_mode(KeyboardMode::None);
        key_window.set_anchor(Edge::Bottom, true);
        key_window.set_anchor(Edge::Right, true);
        key_window.set_margin(Edge::Bottom, 40);
        key_window.set_margin(Edge::Right, 40);
    }
    key_window.add_css_class("keystroke-window");

    let key_box = Box::new(Orientation::Horizontal, 6);
    key_box.set_widget_name("keystroke-container");
    key_window.set_child(Some(&key_box));

    let css_provider = CssProvider::new();
    css_provider.load_from_data("
        window.ime-window, window.keystroke-window {
            background-color: transparent;
        }
        #main-container, #keystroke-container {
            background-color: rgba(20, 20, 20, 0.85);
            border: 1px solid rgba(255, 255, 255, 0.12);
            border-radius: 10px;
            padding: 6px 12px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
        }
        #pinyin-label {
            color: #339af0;
            font-size: 13pt;
            font-weight: 600;
            margin-right: 4px;
            padding-right: 10px;
            border-right: 1px solid rgba(255, 255, 255, 0.1);
        }
        .candidate-item { padding: 2px 8px; border-radius: 6px; }
        .candidate-selected { background-color: #339af0; }
        .candidate-text { color: #f8f9fa; font-size: 14pt; font-weight: 500; }
        .hint-text { color: #adb5bd; font-size: 10pt; margin-left: 4px; }
        .index { font-size: 9pt; color: #6c757d; margin-right: 6px; }
        .key-label {
            background: linear-gradient(to bottom, #444, #222);
            color: #eee;
            font-family: 'Sans', sans-serif;
            font-size: 11pt;
            font-weight: 700;
            padding: 5px 12px;
            border-radius: 6px;
            border: 1px solid #111;
            box-shadow: inset 0 1px 0 rgba(255,255,255,0.1), 0 2px 4px rgba(0,0,0,0.4);
            margin: 2px;
        }
    ");

    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let (tx, gtk_rx) = MainContext::channel::<GuiEvent>(glib::Priority::default());
    
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            let is_exit = matches!(msg, GuiEvent::Exit);
            if tx.send(msg).is_err() || is_exit { break; }
        }
    });

    let window_clone = window.clone();
    let key_window_clone = key_window.clone();
    let pinyin_label_clone = pinyin_label.clone();
    let candidates_box_clone = candidates_box.clone();
    let key_box_clone = key_box.clone();

    gtk_rx.attach(None, move |event| {
        match event {
            GuiEvent::Update { pinyin, candidates, hints, selected } => {
                if pinyin.is_empty() && candidates.is_empty() {
                    window_clone.set_opacity(0.0);
                    window_clone.set_visible(false);
                    while let Some(child) = candidates_box_clone.first_child() {
                        candidates_box_clone.remove(&child);
                    }
                    pinyin_label_clone.set_text("");
                    return glib::Continue(true);
                }
                
                window_clone.set_visible(true);
                window_clone.set_opacity(1.0);
                pinyin_label_clone.set_text(&pinyin);
                while let Some(child) = candidates_box_clone.first_child() {
                    candidates_box_clone.remove(&child);
                }
                
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
                            let hint_label = Label::new(Some(&format!("{}", hint)));
                            hint_label.add_css_class("hint-text");
                            cand_box.append(&hint_label);
                        }
                    }
                    if i == selected { cand_box.add_css_class("candidate-selected"); }
                    candidates_box_clone.append(&cand_box);
                }
            },
            GuiEvent::Keystroke(key_name) => {
                let label = Label::new(Some(&key_name));
                label.add_css_class("key-label");
                key_box_clone.append(&label);
                key_window_clone.set_visible(true);
                key_window_clone.set_opacity(1.0);

                let key_box_weak = key_box_clone.downgrade();
                let label_weak = label.downgrade();
                let key_window_weak = key_window_clone.downgrade();
                
                glib::timeout_add_local(std::time::Duration::from_millis(1000), move || {
                    if let (Some(kb), Some(l)) = (key_box_weak.upgrade(), label_weak.upgrade()) {
                        kb.remove(&l);
                        if kb.first_child().is_none() {
                            if let Some(kw) = key_window_weak.upgrade() { 
                                kw.set_opacity(0.0); 
                                kw.set_visible(false);
                            }
                        }
                    }
                    glib::Continue(false)
                });
            },
            GuiEvent::ClearKeystrokes => {
                while let Some(child) = key_box_clone.first_child() {
                    key_box_clone.remove(&child);
                }
                key_window_clone.set_opacity(0.0);
                key_window_clone.set_visible(false);
            },
            GuiEvent::Exit => {
                window_clone.close();
                key_window_clone.close();
                return glib::Continue(false);
            }
        }
        glib::Continue(true)
    });

    // 初始状态：完全透明但不隐藏，给 Wayland 时间同步状态
    window.set_opacity(0.0);
    // 只有在 LayerShell 模式下才 present，否则普通窗口会弹出
    if is_layer_supported {
        window.present();
    }
    
    key_window.set_opacity(0.0);
    if is_layer_supported {
        key_window.present();
    }

    let loop_ = glib::MainLoop::new(None, false);
    loop_.run();
}
