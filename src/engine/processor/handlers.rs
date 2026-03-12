use std::time::Instant;
use crate::engine::keys::VirtualKey;
use crate::engine::processor::{Processor, Action, Command, FilterMode};
use crate::engine::processor::utils::*;

pub fn handle_idle(processor: &mut Processor, key: VirtualKey, shift_pressed: bool, perform_lookup: bool) -> Action {
    if key == VirtualKey::Enter || key == VirtualKey::Space {
        return Action::PassThrough;
    }
    if is_letter(key) {
        if let Some(c) = key_to_char(key, shift_pressed, processor.caps_lock_enabled) {
            let lang = processor.active_profiles.first().cloned().unwrap_or_default().to_lowercase();
            
            // 1. 尝试新的多维布局 (layouts)
            if let Some(layout) = processor.config.layouts.get(&lang) {
                if let Some(action) = layout.mappings.get(&c.to_string()) {
                    if shift_pressed && !action.shift.is_empty() {
                        return Action::Emit(action.shift.clone());
                    } else if !action.tap.is_empty() {
                        return Action::Emit(action.tap.clone());
                    }
                }
            }

            // 2. 尝试旧的简单映射 (keyboard_layouts)
            if let Some(layout) = processor.config.keyboard_layouts.get(&lang) {
                if let Some(mapped) = layout.get(&c.to_string()) {
                    return Action::Emit(mapped.clone());
                }
            }

            processor.session.push_char(c);
            if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
            if processor.should_block_invalid_input(&processor.session.buffer.clone()) { return Action::Alert; }
            return processor.update_phantom_action();
        }
    }

    if get_punctuation_key(key, shift_pressed).is_some() {
        return processor.handle_punctuation(key, shift_pressed);
    }

    Action::PassThrough
}

pub fn handle_composing(processor: &mut Processor, key: VirtualKey, shift_pressed: bool, perform_lookup: bool) -> Action {
    let mods = crate::engine::ModifierState { shift: shift_pressed, ctrl: false, alt: false, meta: false };
    
    // 1. 优先尝试从 KeyMap 中获取统一指令
    if let Some(cmd) = processor.dispatcher.key_map.get(&(key, mods)).cloned() {
        // 处理方向键交换逻辑 (如果是方向键且启用了交换)
        let final_cmd = if processor.config.swap_arrow_keys {
            match (key, cmd.clone()) {
                (VirtualKey::Up, Command::PrevPage) => Command::PrevCandidate,
                (VirtualKey::Down, Command::NextPage) => Command::NextCandidate,
                (VirtualKey::Left, Command::PrevCandidate) => Command::PrevPage,
                (VirtualKey::Right, Command::NextCandidate) => Command::NextPage,
                _ => cmd
            }
        } else { cmd };
        
        if key == VirtualKey::Space && shift_pressed {
            if let Some(cand) = processor.session.candidates.get(processor.session.selected) {
                if !cand.hint.is_empty() {
                    return processor.commit_candidate(cand.hint.clone(), 99);
                }
            }
        }
        return processor.execute_command(final_cmd);
    }

    // 2. 如果处于导航模式，映射 HJKL
    if processor.session.nav_mode {
        match key {
            VirtualKey::H => return processor.execute_command(Command::PrevCandidate),
            VirtualKey::L => return processor.execute_command(Command::NextCandidate),
            VirtualKey::K => return processor.execute_command(Command::PrevPage),
            VirtualKey::J => return processor.execute_command(Command::NextPage),
            _ => { /* 继续处理其他按键，或退出模式 */ }
        }
    }

    let has_cand = !processor.session.candidates.is_empty();
    let now = Instant::now();

    // --- Shift + Letter 辅助码过滤 / 精确选词 ---
    if is_letter(key) && shift_pressed && !processor.session.buffer.is_empty() {
         if let Some(c) = key_to_char(key, false, processor.caps_lock_enabled) {
             processor.session.shift_used_as_modifier = true;
             // 核心修复：直接设置过滤模式为 Global
             if processor.session.filter_mode != FilterMode::Global {
                 processor.session.filter_mode = FilterMode::Global;
                 // 注意：这里不清除已有的 aux_filter，因为我们可能正在连续输入辅助码
             }
             processor.session.handle_filter_char(c);

             if let Some(act) = processor.lookup() { return act; }
             return processor.update_phantom_action();
         }
    }

    let current_profile = processor.active_profiles.first().cloned().unwrap_or_default();
    if let Some(scheme) = processor.engine.schemes.get(&current_profile) {
        let context = crate::engine::scheme::SchemeContext {
            config: &processor.config.master_config,
            tries: &std::collections::HashMap::new(), 
            syllables: &processor.syllables,
            _user_dict: &processor.config.learned_words,
            active_profiles: &processor.active_profiles,
            candidate_count: processor.session.candidates.len(),
            _filter_mode: processor.session.filter_mode.clone(),
            _aux_filter: &processor.session.aux_filter,
        };
        let act_opt: Option<Action> = scheme.handle_special_key(key, &mut processor.session.buffer, &context);
        if let Some(act) = act_opt {
            if act == Action::Consume {
                if perform_lookup { if let Some(lookup_act) = processor.lookup() { return lookup_act; } }
                return processor.update_phantom_action();
            }
            return act;
        }
    }

    if is_letter(key) {
        if processor.session.filter_mode != FilterMode::None {
            if let Some(c) = key_to_char(key, shift_pressed, processor.caps_lock_enabled) {
                processor.session.handle_filter_char(c);
                if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                return processor.update_phantom_action();
            }
        }
        
        if !shift_pressed && processor.config.enable_double_tap {
            if let Some(last_k) = processor.dispatcher.last_tap_key {
                if last_k == key {
                    if let Some(last_t) = processor.dispatcher.last_tap_time {
                        if now.duration_since(last_t) <= processor.config.double_tap_timeout {
                            if let Some(c) = key_to_char(key, false, processor.caps_lock_enabled) {
                                let lang = processor.active_profiles.first().cloned().unwrap_or_default().to_lowercase();
                                
                                // 1. 尝试 profile 专属的双击映射
                                let mut replacement = None;
                                if let Some(layout) = processor.config.layouts.get(&lang) {
                                    if let Some(action) = layout.mappings.get(&c.to_string()) {
                                        if let Some(dt) = &action.double_tap {
                                            replacement = Some(dt.clone());
                                        }
                                    }
                                }

                                // 2. 尝试全局双击映射
                                if replacement.is_none() {
                                    replacement = processor.config.double_taps.get(&c.to_string()).cloned();
                                }

                                if let Some(r) = replacement {
                                    if processor.session.buffer.ends_with(c) {
                                        processor.session.buffer.pop();
                                        processor.session.buffer.push_str(&r);
                                        processor.dispatcher.last_tap_key = None;
                                        processor.dispatcher.last_tap_time = None;
                                        if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                                        return processor.update_phantom_action();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            processor.dispatcher.last_tap_key = Some(key);
            processor.dispatcher.last_tap_time = Some(now);
        } else {
            processor.dispatcher.last_tap_key = None;
            processor.dispatcher.last_tap_time = None;
        }

        if let Some(c) = key_to_char(key, shift_pressed, processor.caps_lock_enabled) {
            // 如果按下 Shift + 字母输入了大写字母，标记为已作为修饰键使用，防止释放时触发全局过滤
            if shift_pressed {
                processor.session.shift_used_as_modifier = true;
            }
            processor.session.push_char(c);
            if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
            if processor.should_block_invalid_input(&processor.session.buffer.clone()) { return Action::Alert; }
            if let Some(act) = processor.check_auto_commit() { return act; }
            return processor.update_phantom_action();
        }
    } else {
        processor.dispatcher.last_tap_key = None;
        processor.dispatcher.last_tap_time = None;
    }

    if processor.config.page_up_keys.contains(&key) && has_cand {
        return processor.execute_command(Command::PrevPage);
    }
    if processor.config.page_down_keys.contains(&key) && has_cand {
        return processor.execute_command(Command::NextPage);
    }
    if processor.config.prev_candidate_keys.contains(&key) && has_cand {
        return processor.execute_command(Command::PrevCandidate);
    }
    if processor.config.next_candidate_keys.contains(&key) && has_cand {
        return processor.execute_command(Command::NextCandidate);
    }

    if key == VirtualKey::Semicolon && !shift_pressed {
        processor.session.push_char(';');
        if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
        return processor.update_phantom_action();
    }

    match key {
        VirtualKey::Backspace => {
            if processor.session.filter_mode != FilterMode::None {
                processor.session.pop_filter();
                if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                return processor.update_phantom_action();
            }

            if processor.session.buffer.is_empty() {
                processor.commit_history.clear();
                return Action::PassThrough;
            }

            // 修复点：在 pop_char 导致状态重置前，先捕捉当前的 phantom_text 长度
            let old_phantom_len = processor.session.phantom_text.chars().count();
            processor.session.pop_char();

            if processor.session.buffer.is_empty() {
                processor.reset();
                if old_phantom_len > 0 { 
                    Action::DeleteAndEmit { delete: old_phantom_len, insert: "".into() } 
                } else { 
                    Action::Consume 
                }
            } else { 
                if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                processor.update_phantom_action() 
            }
        }

        VirtualKey::Home => { if shift_pressed { processor.session.selected = 0; processor.session.page = 0; } else { processor.session.selected = processor.session.page; } Action::Consume }
        VirtualKey::End => { if has_cand { if shift_pressed { processor.session.selected = processor.session.candidates.len() - 1; processor.session.page = (processor.session.selected / processor.config.page_size) * processor.config.page_size; } else { processor.session.selected = (processor.session.page + processor.config.page_size - 1).min(processor.session.candidates.len() - 1); } } Action::Consume }

        VirtualKey::Apostrophe if !shift_pressed => {
            processor.session.buffer.push('\'');
            processor.session.preview_selected_candidate = false;
            if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
            processor.update_phantom_action()
        }

        VirtualKey::Slash if !processor.session.buffer.is_empty() => {
            let mut new_buffer = processor.session.buffer.clone();
            let last_part_start = new_buffer.rfind(' ').map(|i| i + 1).unwrap_or(0);
            let last_part = &new_buffer[last_part_start..];
            
            let transformed = if last_part.starts_with("zh") {
                last_part.replacen("zh", "z", 1)
            } else if last_part.starts_with("ch") {
                last_part.replacen("ch", "c", 1)
            } else if last_part.starts_with("sh") {
                last_part.replacen("sh", "s", 1)
            } else if last_part.starts_with("z") {
                last_part.replacen("z", "zh", 1)
            } else if last_part.starts_with("c") {
                last_part.replacen("c", "ch", 1)
            } else if last_part.starts_with("s") {
                last_part.replacen("s", "sh", 1)
            } else {
                last_part.to_string()
            };

            if transformed != last_part {
                new_buffer.replace_range(last_part_start.., &transformed);
                processor.session.buffer = new_buffer;
                if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                return processor.update_phantom_action();
            }
            Action::PassThrough
        }

        _ if is_digit(key) => {
            let digit = key_to_digit(key).unwrap_or(0);
            if processor.config.enable_number_selection && processor.config.commit_mode == "single" && digit >= 1 && digit <= processor.config.page_size {
                return processor.execute_command(Command::Select(digit as usize - 1));
            }
            let old_buffer = processor.session.buffer.clone();
            processor.session.push_char(key_to_char(key, false, processor.caps_lock_enabled).unwrap_or('0'));
            if perform_lookup { if let Some(act) = processor.lookup() { return act; } }

            if processor.should_block_invalid_input(&old_buffer) { return Action::Alert; }
            if let Some(act) = processor.check_auto_commit() { return act; } processor.update_phantom_action()
        }
        _ => {
            if get_punctuation_key(key, shift_pressed).is_some() {
                processor.handle_punctuation(key, shift_pressed)
            } else if let Some(c) = key_to_char(key, shift_pressed, processor.caps_lock_enabled) {
                let old_buffer = processor.session.buffer.clone();
                processor.session.push_char(c);
                if perform_lookup { if let Some(act) = processor.lookup() { return act; } }
                if processor.should_block_invalid_input(&old_buffer) { return Action::Alert; }
                if let Some(act) = processor.check_auto_commit() { return act; } processor.update_phantom_action()
            } else { Action::PassThrough }
        }
    }
}
