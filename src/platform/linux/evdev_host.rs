use crate::engine::Processor;
use crate::engine::processor::Action;
use crate::platform::traits::{InputMethodHost, Rect};
use crate::platform::linux::vkbd::Vkbd;
use crate::config::Config;
use crate::ui::GuiEvent;
use evdev::{Device, InputEventKind, Key};
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use crate::config::parse_key;

pub struct EvdevHost {
    processor: Arc<Mutex<Processor>>,
    vkbd: Mutex<Vkbd>,
    dev: Mutex<Device>,
    gui_tx: Option<Sender<GuiEvent>>,
    should_exit: Arc<AtomicBool>,
    config: Arc<std::sync::RwLock<Config>>,
}

impl EvdevHost {
    pub fn new(
        processor: Arc<Mutex<Processor>>, 
        device_path: &str, 
        gui_tx: Option<Sender<GuiEvent>>,
        config: Arc<std::sync::RwLock<Config>>
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dev = Device::open(device_path)?;
        let vkbd = Vkbd::new(&dev)?;
        Ok(Self {
            processor,
            vkbd: Mutex::new(vkbd),
            dev: Mutex::new(dev),
            gui_tx,
            should_exit: Arc::new(AtomicBool::new(false)),
            config,
        })
    }
}

impl InputMethodHost for EvdevHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {
    }

    fn commit_text(&self, text: &str) {
        if let Ok(mut vkbd) = self.vkbd.lock() {
            let _ = vkbd.send_text(text);
        }
    }

    fn get_cursor_rect(&self) -> Option<Rect> {
        None 
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut dev) = self.dev.lock() {
            let _ = dev.grab();
        }
        
        let mut held_keys = HashSet::new();
        println!("[EvdevHost] 启动硬件拦截模式...");

        while !self.should_exit.load(Ordering::Relaxed) {
            let events: Vec<_> = if let Ok(mut dev) = self.dev.lock() {
                dev.fetch_events()?.collect()
            } else {
                break;
            };

            for ev in events {
                if let InputEventKind::Key(key) = ev.kind() {
                    let val = ev.value();
                    if val == 1 { held_keys.insert(key); }
                    else if val == 0 { held_keys.remove(&key); }

                    let conf = self.config.read().unwrap();
                    let toggle_main = parse_key(&conf.hotkeys.switch_language.key);
                    let toggle_alt = parse_key(&conf.hotkeys.switch_language_alt.key);

                    if val == 1 && (is_combo(&held_keys, &toggle_main) || is_combo(&held_keys, &toggle_alt)) {
                        let mut p = self.processor.lock().unwrap();
                        let enabled = p.toggle();
                        println!("[EvdevHost] Toggle Language -> Chinese Enabled: {}", enabled);
                        
                        let msg = if enabled { "中文模式" } else { "英文模式" };
                        let _ = notify_rust::Notification::new()
                            .summary("rust-IME")
                            .body(msg)
                            .timeout(1500)
                            .show();
                        drop(p);
                        self.update_gui();
                        continue;
                    }
                    drop(conf);

                    let shift = held_keys.contains(&Key::KEY_LEFTSHIFT) || held_keys.contains(&Key::KEY_RIGHTSHIFT);
                    
                    let mut p = self.processor.lock().unwrap();
                    if p.chinese_enabled {
                        println!("[EvdevHost] Handling key: {:?} (val: {})", key, val);
                        match p.handle_key(key, val != 0, shift) {
                            Action::Emit(s) => { 
                                println!("[EvdevHost] Emitting text: {}", s);
                                if let Ok(mut vkbd) = self.vkbd.lock() {
                                    let _ = vkbd.send_text(&s); 
                                }
                            }
                            Action::DeleteAndEmit { delete, insert } => { 
                                println!("[EvdevHost] Delete {} and Emit: {}", delete, insert);
                                if let Ok(mut vkbd) = self.vkbd.lock() {
                                    for _ in 0..delete { vkbd.tap(Key::KEY_BACKSPACE); }
                                    let _ = vkbd.send_text(&insert);
                                }
                            }
                            Action::Consume => {
                                println!("[EvdevHost] Consumed key, buffer: {}", p.buffer);
                            }
                            Action::PassThrough => { 
                                if let Ok(mut vkbd) = self.vkbd.lock() {
                                    let _ = vkbd.emit_raw(key, val); 
                                }
                            }
                        }
                        drop(p);
                        self.update_gui();
                    } else {
                        drop(p);
                        if let Ok(mut vkbd) = self.vkbd.lock() {
                            let _ = vkbd.emit_raw(key, val);
                        }
                    }
                }
            }
        }
        
        if let Ok(mut dev) = self.dev.lock() {
            let _ = dev.ungrab();
        }
        Ok(())
    }
}

impl EvdevHost {
    fn update_gui(&self) {
        if let Some(ref tx) = self.gui_tx {
            let p = self.processor.lock().unwrap();
            
            // 如果没开启中文或者 buffer 为空，通常通知 GUI 隐藏
            if !p.chinese_enabled || p.buffer.is_empty() {
                let _ = tx.send(GuiEvent::Update {
                    pinyin: "".into(),
                    candidates: vec![],
                    hints: vec![],
                    selected: 0,
                });
                return;
            }

            let pinyin = if p.best_segmentation.is_empty() { 
                p.buffer.clone() 
            } else { 
                p.best_segmentation.join("'") 
            };
            let _ = tx.send(GuiEvent::Update {
                pinyin,
                candidates: p.candidates.clone(),
                hints: p.candidate_hints.clone(),
                selected: p.selected,
            });
        }
    }
}

fn is_combo(held: &HashSet<Key>, target: &[Key]) -> bool {
    if target.is_empty() { return false; }
    target.iter().all(|k| held.contains(k))
}