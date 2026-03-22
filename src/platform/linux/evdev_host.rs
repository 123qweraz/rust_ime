use crate::engine::keys::VirtualKey;
use crate::engine::processor::Action;
use crate::engine::Processor;
use crate::platform::linux::vkbd::Vkbd;
use crate::platform::traits::{InputMethodHost, Rect};
use crate::ui::GuiEvent;
use evdev::{Device, InputEventKind, Key};
use std::collections::HashSet;
use std::sync::mpsc::Sender;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

fn evdev_to_virtual(key: Key) -> Option<VirtualKey> {
    match key {
        Key::KEY_A => Some(VirtualKey::A),
        Key::KEY_B => Some(VirtualKey::B),
        Key::KEY_C => Some(VirtualKey::C),
        Key::KEY_D => Some(VirtualKey::D),
        Key::KEY_E => Some(VirtualKey::E),
        Key::KEY_F => Some(VirtualKey::F),
        Key::KEY_G => Some(VirtualKey::G),
        Key::KEY_H => Some(VirtualKey::H),
        Key::KEY_I => Some(VirtualKey::I),
        Key::KEY_J => Some(VirtualKey::J),
        Key::KEY_K => Some(VirtualKey::K),
        Key::KEY_L => Some(VirtualKey::L),
        Key::KEY_M => Some(VirtualKey::M),
        Key::KEY_N => Some(VirtualKey::N),
        Key::KEY_O => Some(VirtualKey::O),
        Key::KEY_P => Some(VirtualKey::P),
        Key::KEY_Q => Some(VirtualKey::Q),
        Key::KEY_R => Some(VirtualKey::R),
        Key::KEY_S => Some(VirtualKey::S),
        Key::KEY_T => Some(VirtualKey::T),
        Key::KEY_U => Some(VirtualKey::U),
        Key::KEY_V => Some(VirtualKey::V),
        Key::KEY_W => Some(VirtualKey::W),
        Key::KEY_X => Some(VirtualKey::X),
        Key::KEY_Y => Some(VirtualKey::Y),
        Key::KEY_Z => Some(VirtualKey::Z),
        Key::KEY_0 => Some(VirtualKey::Digit0),
        Key::KEY_1 => Some(VirtualKey::Digit1),
        Key::KEY_2 => Some(VirtualKey::Digit2),
        Key::KEY_3 => Some(VirtualKey::Digit3),
        Key::KEY_4 => Some(VirtualKey::Digit4),
        Key::KEY_5 => Some(VirtualKey::Digit5),
        Key::KEY_6 => Some(VirtualKey::Digit6),
        Key::KEY_7 => Some(VirtualKey::Digit7),
        Key::KEY_8 => Some(VirtualKey::Digit8),
        Key::KEY_9 => Some(VirtualKey::Digit9),
        Key::KEY_SPACE => Some(VirtualKey::Space),
        Key::KEY_ENTER | Key::KEY_KPENTER => Some(VirtualKey::Enter),
        Key::KEY_TAB => Some(VirtualKey::Tab),
        Key::KEY_BACKSPACE => Some(VirtualKey::Backspace),
        Key::KEY_ESC => Some(VirtualKey::Esc),
        Key::KEY_CAPSLOCK => Some(VirtualKey::CapsLock),
        Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => Some(VirtualKey::Shift),
        Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => Some(VirtualKey::Control),
        Key::KEY_LEFTALT | Key::KEY_RIGHTALT => Some(VirtualKey::Alt),
        Key::KEY_LEFT => Some(VirtualKey::Left),
        Key::KEY_RIGHT => Some(VirtualKey::Right),
        Key::KEY_UP => Some(VirtualKey::Up),
        Key::KEY_DOWN => Some(VirtualKey::Down),
        Key::KEY_PAGEUP => Some(VirtualKey::PageUp),
        Key::KEY_PAGEDOWN => Some(VirtualKey::PageDown),
        Key::KEY_HOME => Some(VirtualKey::Home),
        Key::KEY_END => Some(VirtualKey::End),
        Key::KEY_DELETE => Some(VirtualKey::Delete),
        Key::KEY_GRAVE => Some(VirtualKey::Grave),
        Key::KEY_MINUS => Some(VirtualKey::Minus),
        Key::KEY_EQUAL => Some(VirtualKey::Equal),
        Key::KEY_LEFTBRACE => Some(VirtualKey::LeftBrace),
        Key::KEY_RIGHTBRACE => Some(VirtualKey::RightBrace),
        Key::KEY_BACKSLASH => Some(VirtualKey::Backslash),
        Key::KEY_SEMICOLON => Some(VirtualKey::Semicolon),
        Key::KEY_APOSTROPHE => Some(VirtualKey::Apostrophe),
        Key::KEY_COMMA => Some(VirtualKey::Comma),
        Key::KEY_DOT => Some(VirtualKey::Dot),
        Key::KEY_SLASH => Some(VirtualKey::Slash),
        _ => None,
    }
}

pub struct EvdevHost {
    processor: Arc<Mutex<Processor>>,
    vkbd: Arc<Mutex<Vkbd>>,
    dev: Arc<Mutex<Device>>,
    gui_tx: Option<Sender<GuiEvent>>,
    tray_tx: Sender<crate::ui::tray::TrayEvent>,
    should_exit: Arc<AtomicBool>,
    tab_held_and_not_used: bool,
    lookup_tx: std::sync::mpsc::Sender<()>,
    lookup_pending: Arc<AtomicBool>,
}

struct GrabGuard {
    device: Arc<Mutex<Device>>,
}

impl GrabGuard {
    fn new(device: Arc<Mutex<Device>>) -> Self {
        if let Ok(mut dev) = device.lock() {
            if let Err(e) = dev.grab() {
                eprintln!("[EvdevHost] 警告: 无法锁定键盘设备: {e}");
            } else {
                println!("[EvdevHost] 已成功锁定键盘硬件拦截。");
            }
        }
        Self { device }
    }
}

impl Drop for GrabGuard {
    fn drop(&mut self) {
        if let Ok(mut dev) = self.device.lock() {
            let _ = dev.ungrab();
            println!("[EvdevHost] 键盘硬件拦截已安全释放。");
        }
    }
}

impl EvdevHost {
    pub fn new(
        processor: Arc<Mutex<Processor>>,
        device_path: &str,
        gui_tx: Option<Sender<GuiEvent>>,
        tray_tx: Sender<crate::ui::tray::TrayEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let dev = Device::open(device_path)?;
        let vkbd_raw = Vkbd::new(&dev)?;
        let vkbd = Arc::new(Mutex::new(vkbd_raw));
        {
            if let Ok(p) = processor.lock() {
                if let Ok(mut vk) = vkbd.lock() {
                    vk.apply_config(&p.ctx.config.master_config);
                }
            }
        }
        let (lookup_tx, lookup_rx) = std::sync::mpsc::channel::<()>();
        let lookup_pending = Arc::new(AtomicBool::new(false));

        // 启动后台检索线程
        let p_bg = processor.clone();
        let v_bg = vkbd.clone();
        let g_bg = gui_tx.clone();
        let pending_bg = lookup_pending.clone();

        std::thread::spawn(move || {
            while lookup_rx.recv().is_ok() {
                // 消耗掉积压的所有检索请求，只做最后一次
                while lookup_rx.try_recv().is_ok() {}

                if let Ok(mut p) = p_bg.lock() {
                    if let Some(commit_action) = p.lookup() {
                        if let Ok(vkbd) = v_bg.lock() {
                            execute_action(&vkbd, commit_action, None);
                        }
                    }

                    let phantom_action = p.update_phantom_action();
                    if let Ok(vkbd) = v_bg.lock() {
                        execute_action(&vkbd, phantom_action, None);
                    }

                    update_gui_internal(&p, &g_bg);
                }
                pending_bg.store(false, Ordering::SeqCst);
            }
        });

        Ok(Self {
            processor,
            vkbd,
            dev: Arc::new(Mutex::new(dev)),
            gui_tx,
            tray_tx,
            should_exit: Arc::new(AtomicBool::new(false)),
            tab_held_and_not_used: false,
            lookup_tx,
            lookup_pending,
        })
    }
}

impl InputMethodHost for EvdevHost {
    fn set_preedit(&self, _text: &str, _cursor_pos: usize) {}
    fn commit_text(&self, text: &str) {
        if let Ok(vkbd) = self.vkbd.lock() {
            vkbd.send_text(text);
        }
    }

    fn get_cursor_rect(&self) -> Option<Rect> {
        None
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 使用 RAII Guard 自动管理 grab 生命周期
        let _guard = GrabGuard::new(self.dev.clone());
        let mut held_keys = HashSet::new();
        println!("[EvdevHost] 正在运行硬件拦截循环...");

        while !self.should_exit.load(Ordering::Relaxed) {
            let events: Vec<_> = if let Ok(mut dev) = self.dev.lock() {
                match dev.fetch_events() {
                    Ok(evs) => evs.collect(),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                break;
            };

            for ev in events {
                if let InputEventKind::Key(key) = ev.kind() {
                    let val = ev.value();

                    // 1. 基础状态维护
                    if val == 1 {
                        held_keys.insert(key);
                        if key != Key::KEY_TAB {
                            self.tab_held_and_not_used = false;
                        }
                    } else if val == 0 {
                        held_keys.remove(&key);
                    }

                    let shift = held_keys.contains(&Key::KEY_LEFTSHIFT)
                        || held_keys.contains(&Key::KEY_RIGHTSHIFT);
                    let ctrl = held_keys.contains(&Key::KEY_LEFTCTRL)
                        || held_keys.contains(&Key::KEY_RIGHTCTRL);
                    let alt = held_keys.contains(&Key::KEY_LEFTALT)
                        || held_keys.contains(&Key::KEY_RIGHTALT);

                    if let Ok(mut p) = self.processor.lock() {
                        if let Some(vk) = evdev_to_virtual(key) {
                            // 所有的按键（包含组合键）现在都交给 Processor 统一处理
                            let is_sync_key = vk == VirtualKey::Space
                                || vk == VirtualKey::Enter
                                || vk == VirtualKey::CapsLock
                                || vk == VirtualKey::Tab
                                || (vk.to_u32() >= VirtualKey::Digit0.to_u32()
                                    && vk.to_u32() <= VirtualKey::Digit9.to_u32())
                                || matches!(
                                    vk,
                                    VirtualKey::PageUp
                                        | VirtualKey::PageDown
                                        | VirtualKey::Up
                                        | VirtualKey::Down
                                        | VirtualKey::Left
                                        | VirtualKey::Right
                                        | VirtualKey::Minus
                                        | VirtualKey::Equal
                                        | VirtualKey::Comma
                                        | VirtualKey::Dot
                                );

                            if is_sync_key {
                                drop(p);
                                while self.lookup_pending.load(Ordering::SeqCst) {
                                    std::thread::yield_now();
                                }
                                if let Ok(mut p_locked) = self.processor.lock() {
                                    let action =
                                        p_locked.handle_key_ext(vk, val, shift, ctrl, alt, true);

                                    // 如果状态发生了变化，同步到托盘
                                    let enabled = p_locked.ctx.session_state.chinese_enabled;
                                    let profile = p_locked.get_current_profile_display();
                                    let _ =
                                        self.tray_tx.send(crate::ui::tray::TrayEvent::SyncStatus {
                                            chinese_enabled: enabled,
                                            active_profile: profile,
                                        });

                                    if let Ok(vkbd) = self.vkbd.lock() {
                                        execute_action(&vkbd, action, Some((key, val)));
                                    }
                                    if val != 0 {
                                        drop(p_locked);
                                        self.update_gui();
                                    }
                                }
                            } else {
                                let fast_action =
                                    p.handle_key_ext(vk, val, shift, ctrl, alt, false);
                                if let Ok(vkbd) = self.vkbd.lock() {
                                    execute_action(&vkbd, fast_action, Some((key, val)));
                                }

                                if val != 0 {
                                    self.lookup_pending.store(true, Ordering::SeqCst);
                                    let _ = self.lookup_tx.send(());
                                    drop(p);
                                    self.update_gui();
                                }
                            }
                        } else {
                            if let Ok(vkbd) = self.vkbd.lock() {
                                vkbd.emit_raw(key, val);
                            }
                            drop(p);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl EvdevHost {
    fn update_gui(&self) {
        if let Ok(p) = self.processor.lock() {
            update_gui_internal(&p, &self.gui_tx);
        }
    }
}

fn update_gui_internal(p: &Processor, gui_tx: &Option<Sender<GuiEvent>>) {
    if let Some(ref tx) = gui_tx {
        if p.ctx.session.buffer.is_empty() || !p.ctx.session_state.chinese_enabled {
            let _ = tx.send(GuiEvent::Update {
                pinyin: "".into(),
                candidates: vec![],
                selected: 0,
                sentence: "".into(),
                cursor_pos: 0,
                commit_mode: p.ctx.config.commit_mode().to_string(),
            });
            return;
        }

        let pinyin = crate::engine::compositor::Compositor::get_preedit(&p.ctx);

        if p.ctx.config.show_candidates() {
            let page_size = p.ctx.config.page_size();
            let start = p.ctx.session.page.min(p.ctx.session.candidates.len());
            let end = (start + page_size).min(p.ctx.session.candidates.len());

            let mut display_candidates = Vec::new();
            for (i, c) in p.ctx.session.candidates[start..end].iter().enumerate() {
                let label = format!("{}.", i + 1);
                let full_display = if c.hint.is_empty() {
                    format!("{label}{}", c.text)
                } else {
                    format!("{label}{}({})", c.text, c.hint)
                };
                display_candidates.push(crate::ui::DisplayCandidate {
                    text: c.text.to_string(),
                    label,
                    hint: c.hint.to_string(),
                    full_display,
                });
            }

            let relative_selected = p.ctx.session.selected.saturating_sub(start);

            let _ = tx.send(GuiEvent::Update {
                pinyin,
                candidates: display_candidates,
                selected: relative_selected,
                sentence: p.ctx.session.joined_sentence.clone(),
                cursor_pos: p.ctx.session.cursor_pos,
                commit_mode: p.ctx.config.commit_mode().to_string(),
            });
        } else {
            let _ = tx.send(GuiEvent::Update {
                pinyin: "".into(),
                candidates: vec![],
                selected: 0,
                sentence: "".into(),
                cursor_pos: 0,
                commit_mode: p.ctx.config.commit_mode().to_string(),
            });
        }
    }
}

fn execute_action(vkbd: &Vkbd, action: Action, raw_key: Option<(Key, i32)>) {
    match action {
        Action::Emit(s) => {
            vkbd.send_text(&s);
        }
        Action::DeleteAndEmit { delete, insert } => {
            if delete > 0 {
                vkbd.backspace(delete);
            }
            if !insert.is_empty() {
                vkbd.send_text(&insert);
            }
        }
        Action::PassThrough => {
            if let Some((k, v)) = raw_key {
                vkbd.emit_raw(k, v);
            }
        }
        Action::Alert => {
            let root = crate::find_project_root();
            let sound_path = root.join("sounds/beep.wav");
            if sound_path.exists() {
                let _ = std::process::Command::new("canberra-gtk-play")
                    .arg("-f")
                    .arg(sound_path)
                    .spawn();
            } else {
                let _ = std::process::Command::new("canberra-gtk-play")
                    .arg("--id=dialog-error")
                    .spawn();
            }
        }
        _ => {}
    }
}
