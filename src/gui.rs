use gtk::prelude::*;
use gtk::{ApplicationWindow, Label, Box, Orientation, CssProvider, StyleContext};
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
    if gtk::init().is_err() {
        eprintln!("Failed to initialize GTK.");
        return;
    }

    let window = ApplicationWindow::builder()
        .title("Rust IME")
        .decorated(false)
        .skip_taskbar_hint(true)
        .skip_pager_hint(true)
        .type_hint(gdk::WindowTypeHint::Utility)
        .app_paintable(true)
        .accept_focus(false)
        .focus_on_map(false)
        .can_focus(false)
        .build();
    
    window.set_keep_above(true);
    window.set_resizable(false); 
    
    let screen = window.screen().expect("Failed to get screen");
    if let Some(visual) = screen.rgba_visual() {
        window.set_visual(Some(&visual));
    }

    let main_box = Box::new(Orientation::Horizontal, 12);
    main_box.set_widget_name("main-container");
    window.add(&main_box);

    let pinyin_label = Label::new(None);
    pinyin_label.set_widget_name("pinyin-label");
    main_box.pack_start(&pinyin_label, false, false, 5);

    let candidates_box = Box::new(Orientation::Horizontal, 18);
    candidates_box.set_widget_name("candidates-box");
    main_box.pack_start(&candidates_box, true, true, 5);

    let css_provider = CssProvider::new();
    css_provider.load_from_data(b"
        * {
            font-family: 'Inter', 'Segoe UI', 'Noto Sans CJK SC', 'PingFang SC', sans-serif;
        }
        #main-container {
            background-color: rgba(30, 30, 30, 0.92);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            padding: 6px 16px;
            box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
        }
        #pinyin-label {
            color: #339af0;
            font-size: 16pt;
            font-weight: 600;
            margin-right: 4px;
            border-right: 1px solid rgba(255, 255, 255, 0.15);
            padding-right: 14px;
        }
        .candidate-item {
            padding: 2px 10px;
            border-radius: 6px;
            transition: all 0.2s;
        }
        .candidate-selected {
            background-color: #1c7ed6;
            box-shadow: 0 2px 8px rgba(28, 126, 214, 0.4);
        }
        .candidate-text {
            color: #f8f9fa;
            font-size: 18pt;
            font-weight: 500;
        }
        .candidate-selected .candidate-text {
            color: #ffffff;
            font-weight: 600;
        }
        .hint-text {
            color: #adb5bd;
            font-size: 11pt;
            margin-left: 4px;
        }
        .candidate-selected .hint-text {
            color: rgba(255, 255, 255, 0.8);
        }
        .index {
            font-size: 10pt;
            color: #868e96;
            margin-right: 6px;
            font-weight: 400;
        }
        .candidate-selected .index {
            color: rgba(255, 255, 255, 0.7);
        }
    ").expect("Failed to load CSS");

    StyleContext::add_provider_for_screen(
        &screen,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let (tx, gtk_rx) = MainContext::channel::<GuiEvent>(glib::PRIORITY_DEFAULT);
    
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
                gtk::main_quit();
                return glib::Continue(false);
            }
        };

        if pinyin.is_empty() && candidates.is_empty() {
            window_clone.hide();
            return glib::Continue(true);
        }

        pinyin_label_clone.set_text(&pinyin);
        
        candidates_box_clone.children().iter().for_each(|c| candidates_box_clone.remove(c));
        
        let page_start = (selected / 5) * 5;
        let page_end = (page_start + 5).min(candidates.len());

        for i in page_start..page_end {
            let cand_box = Box::new(Orientation::Horizontal, 0);
            cand_box.style_context().add_class("candidate-item");
            
            let idx_label = Label::new(Some(&format!("{}.", (i % 5) + 1)));
            idx_label.style_context().add_class("index");
            
            let val_label = Label::new(Some(&candidates[i]));
            val_label.style_context().add_class("candidate-text");
            
            cand_box.pack_start(&idx_label, false, false, 0);
            cand_box.pack_start(&val_label, false, false, 0);

            if let Some(hint) = hints.get(i) {
                if !hint.is_empty() {
                    let hint_label = Label::new(Some(&format!("({})", hint)));
                    hint_label.style_context().add_class("hint-text");
                    cand_box.pack_start(&hint_label, false, false, 0);
                }
            }
            
            if i == selected {
                cand_box.style_context().add_class("candidate-selected");
            }
            
            candidates_box_clone.pack_start(&cand_box, false, false, 0);
        }

        candidates_box_clone.show_all();
        window_clone.show();
        window_clone.set_keep_above(true);
        
        // Force window to resize to fit content
        window_clone.resize(1, 1); 

        if let Some(gdk_window) = window_clone.window() {
            let display = gdk_window.display();
            let monitor = display.monitor_at_window(&gdk_window).unwrap();
            let monitor_rect = monitor.geometry();
            
            // Get current window size after GTK layout
            let (_, window_height) = window_clone.size();
            let window_width = window_clone.allocated_width();
            
            // Position at bottom center for better visibility if we don't have cursor
            let x = (monitor_rect.width() - window_width) / 2;
            let y = monitor_rect.height() - window_height - 60;
            window_clone.move_(x, y);
        }
        
        glib::Continue(true)
    });

    window.show_all();
    window.hide();

    gtk::main();
}
