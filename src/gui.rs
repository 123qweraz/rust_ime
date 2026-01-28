use gtk::prelude::*;
use gtk::{ApplicationWindow, Label, Box, Orientation, CssProvider, StyleContext};
use std::sync::mpsc::Receiver;
use glib::MainContext;

pub fn start_gui(rx: Receiver<(String, Vec<String>, usize)>) {
    if gtk::init().is_err() {
        eprintln!("Failed to initialize GTK.");
        return;
    }

    let window = ApplicationWindow::builder()
        .title("Rust IME")
        .decorated(false)
        .skip_taskbar_hint(true)
        .skip_pager_hint(true)
        .type_hint(gdk::WindowTypeHint::Menu)
        .app_paintable(true)
        .accept_focus(false)
        .can_focus(false)
        .build();
    
    // 显式确保这些属性被应用
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
        #main-container {
            background-color: rgba(25, 25, 25, 0.85);
            border: 1px solid rgba(255, 255, 255, 0.15);
            border-radius: 8px;
            padding: 4px 12px;
            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
        }
        #pinyin-label {
            color: #74c0fc;
            font-size: 15pt;
            font-weight: bold;
            margin-right: 8px;
            border-right: 1px solid rgba(255, 255, 255, 0.2);
            padding-right: 12px;
        }
        .candidate {
            color: #ced4da;
            font-size: 16pt;
        }
        .candidate-selected {
            color: #ffffff;
            background-color: #1c7ed6;
            border-radius: 4px;
            padding: 0 8px;
            font-weight: bold;
        }
        .index {
            font-size: 10pt;
            color: #868e96;
            margin-right: 4px;
        }
    ").expect("Failed to load CSS");

    StyleContext::add_provider_for_screen(
        &screen,
        &css_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let (tx, gtk_rx) = MainContext::channel::<(String, Vec<String>, usize)>(glib::PRIORITY_DEFAULT);
    
    std::thread::spawn(move || {
        while let Ok(msg) = rx.recv() {
            if tx.send(msg).is_err() {
                break;
            }
        }
    });

    let window_clone = window.clone();
    let pinyin_label_clone = pinyin_label.clone();
    let candidates_box_clone = candidates_box.clone();

    gtk_rx.attach(None, move |(pinyin, candidates, selected)| {
        if pinyin.is_empty() && candidates.is_empty() {
            window_clone.hide();
            return glib::Continue(true);
        }

        pinyin_label_clone.set_text(&pinyin);
        
        candidates_box_clone.children().iter().for_each(|c| candidates_box_clone.remove(c));
        
        let page_start = (selected / 5) * 5;
        let page_end = (page_start + 5).min(candidates.len());

        for i in page_start..page_end {
            let cand_box = Box::new(Orientation::Horizontal, 2);
            let idx_label = Label::new(Some(&format!("{}.", (i % 5) + 1)));
            idx_label.style_context().add_class("index");
            
            let val_label = Label::new(Some(&candidates[i]));
            val_label.style_context().add_class("candidate");
            
            if i == selected {
                cand_box.style_context().add_class("candidate-selected");
            }
            
            cand_box.pack_start(&idx_label, false, false, 0);
            cand_box.pack_start(&val_label, false, false, 0);
            candidates_box_clone.pack_start(&cand_box, false, false, 0);
        }

        candidates_box_clone.show_all();
        window_clone.show();
        window_clone.set_keep_above(true);
        
        if let Some(gdk_window) = window_clone.window() {
            let display = gdk_window.display();
            let monitor = display.monitor_at_window(&gdk_window).unwrap();
            let rect = monitor.geometry();
            // Move to bottom right for now, as we don't have cursor position yet
            window_clone.move_(rect.width() - 650, rect.height() - 100);
        }
        
        glib::Continue(true)
    });

    window.show_all();
    window.hide();

    gtk::main();
}
