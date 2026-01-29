use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Label, Box, Orientation, CssProvider};
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
    Exit,
}

pub fn start_gui(rx: Receiver<GuiEvent>) {
    if gtk4::init().is_err() {
        eprintln!("Failed to initialize GTK4.");
        return;
    }

    let window = ApplicationWindow::builder()
        .title("Rust IME")
        .decorated(false)
        .can_focus(false)
        .focusable(false)
        .build();
    
    // Set window as non-focusable and utility-like
    // Note: GTK4 position management is different, but we try our best to keep it floating
    
    let main_box = Box::new(Orientation::Horizontal, 8);
    main_box.set_widget_name("main-container");
    window.set_child(Some(&main_box));

    let pinyin_label = Label::new(None);
    pinyin_label.set_widget_name("pinyin-label");
    main_box.append(&pinyin_label);

    let candidates_box = Box::new(Orientation::Horizontal, 12);
    candidates_box.set_widget_name("candidates-box");
    main_box.append(&candidates_box);

    let css_provider = CssProvider::new();
    // Compact style, semi-transparent background, smaller fonts
    css_provider.load_from_data("
        * {
            font-family: 'Inter', 'Segoe UI', 'Noto Sans CJK SC', 'PingFang SC', sans-serif;
        }
        window {
            background-color: transparent;
        }
        #main-container {
            background-color: rgba(35, 35, 35, 0.85);
            border: 1px solid rgba(255, 255, 255, 0.08);
            border-radius: 8px;
            padding: 4px 10px;
        }
        #pinyin-label {
            color: #4dabf7;
            font-size: 13pt;
            font-weight: 500;
            margin-right: 2px;
            border-right: 1px solid rgba(255, 255, 255, 0.1);
            padding-right: 8px;
        }
        .candidate-item {
            padding: 1px 6px;
            border-radius: 4px;
        }
        .candidate-selected {
            background-color: #339af0;
        }
        .candidate-text {
            color: #e9ecef;
            font-size: 14pt;
            font-weight: 500;
        }
        .candidate-selected .candidate-text {
            color: #ffffff;
            font-weight: 600;
        }
        .hint-text {
            color: #adb5bd;
            font-size: 9pt;
            margin-left: 2px;
        }
        .candidate-selected .hint-text {
            color: rgba(255, 255, 255, 0.8);
        }
        .index {
            font-size: 8pt;
            color: #868e96;
            margin-right: 4px;
        }
        .candidate-selected .index {
            color: rgba(255, 255, 255, 0.7);
        }
    ");

    if let Some(display) = gdk4::Display::default() {
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
            if tx.send(msg).is_err() || is_exit {
                break;
            }
        }
    });

    let window_clone = window.clone();
    let pinyin_label_clone = pinyin_label.clone();
    let candidates_box_clone = candidates_box.clone();

    gtk_rx.attach(None, move |event| {
        let (pinyin, candidates, hints, selected) = match event {
            GuiEvent::Update { pinyin, candidates, hints, selected } => (pinyin, candidates, hints, selected),
            GuiEvent::Exit => {
                window_clone.close();
                return glib::Continue(false);
            }
        };

        if pinyin.is_empty() && candidates.is_empty() {
            window_clone.set_visible(false);
            return glib::Continue(true);
        }

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
            
            if i == selected {
                cand_box.add_css_class("candidate-selected");
            }
            
            candidates_box_clone.append(&cand_box);
        }

        window_clone.set_visible(true);
        window_clone.present();
        
        glib::Continue(true)
    });

    window.set_visible(false);

    let loop_ = glib::MainLoop::new(None, false);
    loop_.run();
}
