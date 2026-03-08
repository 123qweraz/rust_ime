use std::time::Instant;
use crate::engine::keys::VirtualKey;
use crate::engine::processor::{Processor, Action, ImeState};
use crate::engine::processor::utils::*;

pub fn process_modifiers(processor: &mut Processor, key: VirtualKey, is_press: bool, is_release: bool) -> Option<Action> {
    if is_press && key == VirtualKey::Shift {
        processor.session.shift_used_as_modifier = false;
    }

    if is_release {
        if key == VirtualKey::Shift {
            if !processor.session.buffer.is_empty() {
                if !processor.session.shift_used_as_modifier {
                    processor.start_global_filter();
                }
                processor.session.shift_used_as_modifier = false;
                return Some(Action::Consume);
            }
            processor.session.shift_used_as_modifier = false;
        }
        if processor.session.buffer.is_empty() { return Some(Action::PassThrough); }
        return Some(Action::Consume);
    }

    if key == VirtualKey::CapsLock && is_press {
        // CapsLock 的按下逻辑已移至 handle_key_ext
        return None;
    }
    
    None
}

pub fn process_intent(processor: &mut Processor, key: VirtualKey, val: i32, shift_pressed: bool, now: Instant) -> Option<Action> {
    let is_repeat = val == 2;
    let is_release = val == 0;

    if ((processor.config.enable_long_press && is_letter(key)) || (processor.config.enable_punctuation_long_press && get_punctuation_key(key, shift_pressed).is_some()))
        && !shift_pressed {
            if val == 1 {
                processor.dispatcher.key_press_info = Some((key, now));
                processor.dispatcher.long_press_triggered = false;
            } else if is_repeat {
                if !processor.dispatcher.long_press_triggered {
                    if let Some((press_key, press_time)) = processor.dispatcher.key_press_info {
                        if press_key == key && now.duration_since(press_time) >= processor.config.long_press_timeout {
                            if is_letter(key) {
                                if let Some(c) = key_to_char(key, false, processor.caps_lock_enabled) {
                                    if let Some(replacement) = processor.config.long_press_mappings.get(&c.to_string()).cloned() {
                                        processor.dispatcher.long_press_triggered = true;
                                        if !processor.session.buffer.is_empty() {
                                            if let Some(last_char) = processor.session.buffer.chars().last() {
                                                if last_char.to_string() == c.to_string() {
                                                    processor.session.buffer.pop();
                                                }
                                            }
                                        }
                                        return Some(processor.inject_text(&replacement));
                                    }
                                }
                            } else if let Some(p_key) = get_punctuation_key(key, false) {
                                if let Some(replacement) = processor.config.punctuation_long_press_mappings.get(p_key).cloned() {
                                    processor.dispatcher.long_press_triggered = true;
                                    let mut commit_text = if !processor.session.joined_sentence.is_empty() { 
                                        processor.session.joined_sentence.trim_end().to_string() 
                                    } else if !processor.session.candidates.is_empty() { 
                                        processor.session.candidates[0].text.trim_end().to_string() 
                                    } else { 
                                        processor.session.buffer.trim_end().to_string() 
                                    };
                                    commit_text.push_str(&replacement);
                                    let del_len = processor.session.phantom_text.chars().count();
                                    processor.clear_composing();
                                    processor.commit_history.clear(); 
                                    return Some(Action::DeleteAndEmit { delete: del_len, insert: commit_text });
                                }
                            }
                        }
                    }
                }
                return Some(Action::Consume); 
            } else if is_release {
                processor.dispatcher.key_press_info = None;
                if processor.dispatcher.long_press_triggered {
                    return Some(Action::Consume); 
                }
            }
    }
    None
}

pub fn process_switch_mode(processor: &mut Processor, key: VirtualKey, is_press: bool, is_release: bool) -> Option<Action> {
    if !processor.session.switch_mode { return None; }
    
    if is_press {
        match key {
            VirtualKey::Esc | VirtualKey::Space | VirtualKey::Enter => { 
                processor.session.switch_mode = false; 
                return Some(Action::Notify("快捷切换".into(), "已退出".into())); 
            }
            VirtualKey::E => {
                processor.session.switch_mode = false;
                if let Some((pinyin, word)) = processor.commit_history.pop() {
                    let del_count = word.chars().count();
                    processor.session.buffer = pinyin;
                    processor.session.state = ImeState::Composing;
                    let _ = processor.lookup();
                    return Some(Action::DeleteAndEmit { delete: del_count, insert: "".into() });
                }
                return Some(Action::Consume);
            }
            VirtualKey::Z => {
                processor.session.switch_mode = false;
                if processor.engine.trie_paths.contains_key("english") {
                    processor.active_profiles = vec!["english".to_string()];
                    processor.reset();
                    return Some(Action::Notify("英".into(), "已切换至英语方案".into()));
                }
                return Some(Action::Consume);
            }
            _ if is_letter(key) => {
                let k = key_to_char(key, false, processor.caps_lock_enabled).unwrap_or(' ').to_string();
                let mut target_profile = None;
                for (trigger_key, profile_name) in &processor.config.profile_keys {
                    if trigger_key == &k { target_profile = Some(profile_name.clone()); break; }
                }

                if let Some(p_str) = target_profile {
                    let profiles: Vec<String> = p_str.split(',').map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty() && processor.engine.trie_paths.contains_key(s)).collect();
                    if !profiles.is_empty() {
                        processor.active_profiles = profiles;
                        let display = processor.get_current_profile_display();
                        let short_display = processor.get_short_display();
                        let _ = processor.lookup();
                        processor.session.switch_mode = false;
                        return Some(Action::Notify(short_display, format!("方案: {}", display)));
                    } else {
                        processor.session.switch_mode = false;
                        return Some(Action::Notify("❌".into(), format!("错误: 方案 [{}] 的词库未加载", p_str)));
                    }
                }
            }
            _ => {} 
        }
        return Some(Action::Consume);
    }

    if is_release {
        return Some(Action::Consume);
    }
    
    None
}
