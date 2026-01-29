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
        .build();
    
    // In GTK4, some hints and properties are set differently or have moved
    // Position/Type hints are more restricted, especially on Wayland.
    
    let main_box = Box::new(Orientation::Horizontal, 12);
    main_box.set_widget_name("main-container");
    window.set_child(Some(&main_box));

    let pinyin_label = Label::new(None);
    pinyin_label.set_widget_name("pinyin-label");
    main_box.append(&pinyin_label);

    let candidates_box = Box::new(Orientation::Horizontal, 18);
    candidates_box.set_widget_name("candidates-box");
    main_box.append(&candidates_box);

    let css_provider = CssProvider::new();
    css_provider.load_from_data("
        * {
            font-family: 'Inter', 'Segoe UI', 'Noto Sans CJK SC', 'PingFang SC', sans-serif;
        }
        #main-container {
            background-color: rgba(30, 30, 30, 0.92);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 10px;
            padding: 6px 16px;
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
        }
        .candidate-selected {
            background-color: #1c7ed6;
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
                // GTK4 exit handling might be different if using Application, 
                // but here we can just stop the loop or hide.
                window_clone.close();
                return glib::Continue(false);
            }
        };

        if pinyin.is_empty() && candidates.is_empty() {
            window_clone.set_visible(false);
            return glib::Continue(true);
        }

        pinyin_label_clone.set_text(&pinyin);
        
        // GTK4: Removing children is different. We can use remove() on each child.
        while let Some(child) = candidates_box_clone.first_child() {
            candidates_box_clone.remove(&child);
        }
        
        let page_start = (selected / 5) * 5;
        let page_end = (page_start + 5).min(candidates.len());

        for i in page_start..page_end {
            let cand_box = Box::new(Orientation::Horizontal, 0);
            cand_box.add_css_class("candidate-item");
            
            let idx_label = Label::new(Some(&format!("{}.", (i % 5) + 1)));
            idx_label.add_css_class("index");
            
            let val_label = Label::new(Some(&candidates[i]));
            val_label.add_css_class("candidate-text");
            
            cand_box.append(&idx_label);
            cand_box.append(&val_label);

            if let Some(hint) = hints.get(i) {
                if !hint.is_empty() {
                    let hint_label = Label::new(Some(&format!("({})", hint)));
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
        // Position management in GTK4 is harder. 
        // window.move_() is removed. For now, we just present.
        window_clone.present();
        
        glib::Continue(true)
    });

    // In GTK4, widgets are visible by default, but windows need to be presented.
    // However, we start hidden.
    window.set_visible(false);

    // GTK4 doesn't have gtk::main(). It uses a different loop structure usually 
    // via Application, but we can still use a manual loop or glib::MainLoop.
    let loop_ = glib::MainLoop::new(None, false);
    loop_.run();
}