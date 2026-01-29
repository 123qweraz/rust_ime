use gtk4::prelude::*;
use gtk4::{Window, Label, Box, Orientation, CssProvider, GestureClick};
use gdk4::Display;
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
    Exit,
}

pub fn start_gui(rx: Receiver<GuiEvent>) {
    if gtk4::init().is_err() {
        eprintln!("Failed to initialize GTK4.");
        return;
    }

    // --- Candidate Window ---
    let window = Window::builder()
        .title("Rust IME")
        .decorated(false)
        .can_focus(false)
        .focusable(false)
        .resizable(false)
        .build();
    
    window.set_property("deletable", false);
    window.add_css_class("ime-window");
    
    let main_box = Box::new(Orientation::Horizontal, 8);
    main_box.set_widget_name("main-container");
    window.set_child(Some(&main_box));

    // Add drag support to the main container
    let drag_gesture = GestureClick::new();
    let window_clone_for_drag = window.clone();
    drag_gesture.connect_pressed(move |gesture, _n, x, y| {
        let surface = window_clone_for_drag.surface();
        if let Some(display) = Display::default() {
            if let Some(seat) = display.default_seat() {
                if let Some(device) = seat.pointer() {
                    if let Ok(toplevel) = surface.dynamic_cast::<gdk4::Toplevel>() {
                        gesture.set_state(gtk4::EventSequenceState::Claimed);
                        toplevel.begin_move(&device, 1, x, y, 0);
                    }
                }
            }
        }
    });
    main_box.add_controller(drag_gesture);

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
        .build();
    key_window.set_property("deletable", false);
    key_window.add_css_class("keystroke-window");

    let key_box = Box::new(Orientation::Horizontal, 6);
    key_box.set_widget_name("keystroke-container");
    key_window.set_child(Some(&key_box));

    let css_provider = CssProvider::new();
    css_provider.load_from_data("
        window.ime-window, window.keystroke-window {
            background-color: transparent;
            margin: 0;
            padding: 0;
            box-shadow: none;
            border: none;
        }
        window.ime-window decoration, window.keystroke-window decoration,
        window.ime-window headerbar, window.keystroke-window headerbar,
        window.ime-window titlebar, window.keystroke-window titlebar,
        window.ime-window windowhandle, window.keystroke-window windowhandle,
        window.ime-window button.titlebutton, window.keystroke-window button.titlebutton {
            opacity: 0;
            margin: 0;
            padding: 0;
            min-height: 0;
            min-width: 0;
        }
        #main-container {
            background-color: rgba(30, 30, 30, 0.88);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 6px;
            padding: 4px 10px;
            margin: 0;
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

        /* Keystroke Styles */
        #keystroke-container {
            padding: 10px;
        }
        .key-label {
            background-color: rgba(40, 40, 40, 0.9);
            color: white;
            font-family: 'Monospace';
            font-size: 12pt;
            font-weight: bold;
            padding: 6px 12px;
            border-radius: 6px;
            border: 1px solid rgba(255, 255, 255, 0.2);
            box-shadow: 0 2px 4px rgba(0,0,0,0.3);
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
    let key_window_clone = key_window.clone();
    let pinyin_label_clone = pinyin_label.clone();
    let candidates_box_clone = candidates_box.clone();
    let key_box_clone = key_box.clone();

    gtk_rx.attach(None, move |event| {
        match event {
            GuiEvent::Update { pinyin, candidates, hints, selected } => {
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
            },
            GuiEvent::Keystroke(key_name) => {
                let label = Label::new(Some(&key_name));
                label.add_css_class("key-label");
                key_box_clone.append(&label);
                
                if !key_window_clone.is_visible() {
                    key_window_clone.set_visible(true);
                }

                // Remove after 2 seconds
                let key_box_weak = key_box_clone.downgrade();
                let label_weak = label.downgrade();
                let key_window_weak = key_window_clone.downgrade();
                
                glib::timeout_add_local(std::time::Duration::from_millis(2000), move || {
                    if let (Some(kb), Some(l)) = (key_box_weak.upgrade(), label_weak.upgrade()) {
                        kb.remove(&l);
                        // Hide window if empty
                        if kb.first_child().is_none() {
                            if let Some(kw) = key_window_weak.upgrade() {
                                kw.set_visible(false);
                            }
                        }
                    }
                    glib::Continue(false)
                });
            },
            GuiEvent::Exit => {
                window_clone.close();
                key_window_clone.close();
                return glib::Continue(false);
            }
        }
        
        glib::Continue(true)
    });

    window.set_visible(false);
    key_window.set_visible(false);

    let loop_ = glib::MainLoop::new(None, false);
    loop_.run();
}