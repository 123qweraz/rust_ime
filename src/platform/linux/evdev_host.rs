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
    processor: Processor,
    vkbd: Mutex<Vkbd>,
    dev: Mutex<Device>,
    gui_tx: Option<Sender<GuiEvent>>,
    should_exit: Arc<AtomicBool>,
    config: Arc<std::sync::RwLock<Config>>,
}

impl EvdevHost {
    pub fn new(
        processor: Processor, 
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
                    if val == 1 && is_combo(&held_keys, &parse_key(&conf.hotkeys.switch_language.key)) {
                        let enabled = self.processor.toggle();
                        println!("[EvdevHost] Toggle Language -> Chinese Enabled: {}", enabled);
                        self.update_gui();
                        continue;
                    }
                    drop(conf);

                    let shift = held_keys.contains(&Key::KEY_LEFTSHIFT) || held_keys.contains(&Key::KEY_RIGHTSHIFT);
                    
                    if self.processor.chinese_enabled {
                        println!("[EvdevHost] Handling key: {:?} (val: {})", key, val);
                        match self.processor.handle_key(key, val != 0, shift) {
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
                                println!("[EvdevHost] Consumed key, buffer: {}", self.processor.buffer);
                            }
                            Action::PassThrough => { 
                                if let Ok(mut vkbd) = self.vkbd.lock() {
                                    let _ = vkbd.emit_raw(key, val); 
                                }
                            }
                        }
                        self.update_gui();
                    } else {
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
            let pinyin = if self.processor.best_segmentation.is_empty() { 
                self.processor.buffer.clone() 
            } else { 
                self.processor.best_segmentation.join("'") 
            };
            let _ = tx.send(GuiEvent::Update {
                pinyin,
                candidates: self.processor.candidates.clone(),
                hints: self.processor.candidate_hints.clone(),
                selected: self.processor.selected,
            });
        }
    }
}

fn is_combo(held: &HashSet<Key>, target: &[Key]) -> bool {
    if target.is_empty() { return false; }
    target.iter().all(|k| held.contains(k))
}