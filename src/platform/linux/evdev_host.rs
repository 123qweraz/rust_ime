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
use crate::NotifyEvent;

pub struct EvdevHost {
    processor: Arc<Mutex<Processor>>,
    vkbd: Mutex<Vkbd>,
    dev: Mutex<Device>,
    gui_tx: Option<Sender<GuiEvent>>,
    notify_tx: Sender<NotifyEvent>,
    should_exit: Arc<AtomicBool>,
    config: Arc<std::sync::RwLock<Config>>,
}

impl EvdevHost {
    pub fn new(
        processor: Arc<Mutex<Processor>>, 
        device_path: &str, 
        gui_tx: Option<Sender<GuiEvent>>,
        config: Arc<std::sync::RwLock<Config>>,
        notify_tx: Sender<NotifyEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dev = Device::open(device_path)?;
        let vkbd = Vkbd::new(&dev)?;
        Ok(Self {
            processor,
            vkbd: Mutex::new(vkbd),
            dev: Mutex::new(dev),
            gui_tx,
            notify_tx,
            should_exit: Arc::new(AtomicBool::new(false)),
            config,
        })
    }
}

impl InputMethodHost for EvdevHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {}
    fn commit_text(&self, text: &str) {
        if let Ok(mut vkbd) = self.vkbd.lock() { let _ = vkbd.send_text(text); }
    }
    fn get_cursor_rect(&self) -> Option<Rect> { None }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut dev) = self.dev.lock() { let _ = dev.grab(); }
        let mut held_keys = HashSet::new();
        println!("[EvdevHost] 启动硬件拦截模式...");

        while !self.should_exit.load(Ordering::Relaxed) {
            let events: Vec<_> = if let Ok(mut dev) = self.dev.lock() {
                dev.fetch_events()?.collect()
            } else { break; };

            for ev in events {
                if let InputEventKind::Key(key) = ev.kind() {
                    let val = ev.value();
                    if val == 1 { 
                        held_keys.insert(key); 
                        let show_ks = self.processor.lock().unwrap().show_keystrokes;
                        if show_ks {
                            if let Some(ref tx) = self.gui_tx {
                                let name = format!("{:?}", key).replace("KEY_", "");
                                let _ = tx.send(GuiEvent::Keystroke(name));
                            }
                        }
                    } else if val == 0 { held_keys.remove(&key); }

                    let conf = self.config.read().unwrap();
                    let toggle_main = parse_key(&conf.hotkeys.switch_language.key);
                    let toggle_alt = parse_key(&conf.hotkeys.switch_language_alt.key);

                    if val == 1 && (is_combo(&held_keys, &toggle_main) || is_combo(&held_keys, &toggle_alt)) {
                        let mut p = self.processor.lock().unwrap();
                        let enabled = p.toggle();
                        let msg = if enabled { "中文模式" } else { "英文模式" };
                        let _ = self.notify_tx.send(NotifyEvent::Message(msg.to_string()));
                        drop(p);
                        self.update_gui();
                        continue;
                    }
                    drop(conf);

                    let shift = held_keys.contains(&Key::KEY_LEFTSHIFT) || held_keys.contains(&Key::KEY_RIGHTSHIFT);
                    let mut p = self.processor.lock().unwrap();
                    if p.chinese_enabled {
                        match p.handle_key(key, val != 0, shift) {
                            Action::Emit(s) => { if let Ok(mut vkbd) = self.vkbd.lock() { let _ = vkbd.send_text(&s); } }
                            Action::DeleteAndEmit { delete, insert } => { 
                                if let Ok(mut vkbd) = self.vkbd.lock() {
                                    for _ in 0..delete { vkbd.tap(Key::KEY_BACKSPACE); }
                                    let _ = vkbd.send_text(&insert);
                                }
                            }
                            Action::Consume => {}
                            Action::PassThrough => { if let Ok(mut vkbd) = self.vkbd.lock() { let _ = vkbd.emit_raw(key, val); } }
                        }
                        drop(p);
                        self.update_gui();
                        self.notify_preview();
                    } else {
                        drop(p);
                        if let Ok(mut vkbd) = self.vkbd.lock() { let _ = vkbd.emit_raw(key, val); }
                    }
                }
            }
        }
        if let Ok(mut dev) = self.dev.lock() { let _ = dev.ungrab(); }
        Ok(())
    }
}

impl EvdevHost {
    fn update_gui(&self) {
        if let Some(ref tx) = self.gui_tx {
            let p = self.processor.lock().unwrap();
            
            // 如果不显示候选框，且预览模式为 None，则清空 GUI
            if !p.show_candidates && p.phantom_mode == engine::processor::PhantomMode::None {
                let _ = tx.send(GuiEvent::Update { pinyin: "".into(), candidates: vec![], hints: vec![], selected: 0 });
                return;
            }

            if !p.chinese_enabled || p.buffer.is_empty() {
                let _ = tx.send(GuiEvent::Update { pinyin: "".into(), candidates: vec![], hints: vec![], selected: 0 });
                return;
            }

            let pinyin = if p.best_segmentation.is_empty() { p.buffer.clone() } else { p.best_segmentation.join("'") };
            
            // 如果开启了候选框显示，发送完整数据
            if p.show_candidates {
                let _ = tx.send(GuiEvent::Update { pinyin, candidates: p.candidates.clone(), hints: p.candidate_hints.clone(), selected: p.selected });
            } else {
                // 仅预览模式：只发送拼音，不发送候选词
                let _ = tx.send(GuiEvent::Update { pinyin, candidates: vec![], hints: vec![], selected: 0 });
            }
        }
    }

    fn notify_preview(&self) {
        let p = self.processor.lock().unwrap();
        if !p.show_notifications || p.buffer.is_empty() { 
            let _ = self.notify_tx.send(NotifyEvent::Close);
            return; 
        }
        let pinyin = if p.best_segmentation.is_empty() { p.buffer.clone() } else { p.best_segmentation.join("'") };
        let mut body = String::new();
        let start = p.page;
        let end = (start + 5).min(p.candidates.len());
        for (i, cand) in p.candidates[start..end].iter().enumerate() {
            let abs_idx = start + i;
            let hint = p.candidate_hints.get(abs_idx).cloned().unwrap_or_default();
            if abs_idx == p.selected { body.push_str(&format!("【{}.{}{}】 ", i+1, cand, hint)); }
            else { body.push_str(&format!("{}.{}{} ", i+1, cand, hint)); }
        }
        let _ = self.notify_tx.send(NotifyEvent::Update(format!("拼音: {}", pinyin), body));
    }
}

fn is_combo(held: &HashSet<Key>, target: &[Key]) -> bool {
    if target.is_empty() { return false; }
    target.iter().all(|k| held.contains(k))
}
