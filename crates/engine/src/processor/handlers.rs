use crate::compositor::{should_block_invalid_input, Compositor};
use crate::keys::VirtualKey;
use crate::pipeline::lookup;
use crate::processor::commands;
use crate::processor::utils::*;
use crate::processor::{Action, Command, FilterMode};
use crate::EngineContext;
use std::time::Instant;

pub fn handle_idle(
    ctx: &mut EngineContext,
    key: VirtualKey,
    shift_pressed: bool,
    perform_lookup: bool,
) -> Action {
    if key == VirtualKey::Enter || key == VirtualKey::Space {
        return Action::PassThrough;
    }
    if is_letter(key) {
        if let Some(c) = key_to_char(key, shift_pressed, ctx.session_state.caps_lock_enabled) {
            let lang = ctx
                .session_state
                .active_profiles
                .first()
                .cloned()
                .unwrap_or_default()
                .to_lowercase();

            if let Some(layout) = ctx.config.layouts().get(&lang) {
                if let Some(action) = layout.mappings.get(&c.to_string()) {
                    if shift_pressed && !action.shift.is_empty() {
                        return Action::Emit(action.shift.clone());
                    } else if !action.tap.is_empty() {
                        return Action::Emit(action.tap.clone());
                    }
                }
            }

            if let Some(layout) = ctx.config.keyboard_layouts().get(&lang) {
                if let Some(mapped) = layout.get(&c.to_string()) {
                    return Action::Emit(mapped.clone());
                }
            }

            ctx.session.push_char(c);
            if perform_lookup {
                if let Some(act) = lookup(ctx) {
                    return act;
                }
            }
            if should_block_invalid_input(ctx, &ctx.session.buffer.clone()) {
                return Action::Alert;
            }
            let _ = Compositor::update_phantom_action(ctx);
        }
    }

    if get_punctuation_key(key, shift_pressed).is_some() {
        return handle_punctuation(ctx, key, shift_pressed);
    }

    Action::PassThrough
}

pub fn handle_composing(
    ctx: &mut EngineContext,
    key: VirtualKey,
    shift_pressed: bool,
    perform_lookup: bool,
) -> Action {
    let mods = crate::ModifierState {
        shift: shift_pressed,
        ctrl: false,
        alt: false,
        meta: false,
    };

    if let Some(cmd) = ctx.dispatcher.key_map.get(&(key, mods)).cloned() {
        let final_cmd = if ctx.config.swap_arrow_keys() {
            match (key, cmd.clone()) {
                (VirtualKey::Up, Command::PrevPage) => Command::PrevCandidate,
                (VirtualKey::Down, Command::NextPage) => Command::NextCandidate,
                (VirtualKey::Left, Command::PrevCandidate) => Command::PrevPage,
                (VirtualKey::Right, Command::NextPage) => Command::NextPage,
                _ => cmd,
            }
        } else {
            cmd
        };

        if key == VirtualKey::Space && shift_pressed {
            if let Some(cand) = ctx.session.candidates.get(ctx.session.selected) {
                if !cand.hint.is_empty() {
                    return commands::commit_candidate(ctx, cand.hint.clone(), 99);
                }
            }
        }
        return commands::execute_command(ctx, final_cmd);
    }

    if ctx.session.nav_mode || ctx.session_state.capslock_down {
        match key {
            VirtualKey::H => return commands::execute_command(ctx, Command::PrevCandidate),
            VirtualKey::L => return commands::execute_command(ctx, Command::NextCandidate),
            VirtualKey::K => return commands::execute_command(ctx, Command::PrevPage),
            VirtualKey::J => return commands::execute_command(ctx, Command::NextPage),
            _ => {}
        }
    }

    let has_cand = !ctx.session.candidates.is_empty();
    let now = Instant::now();

    if is_letter(key) && shift_pressed && !ctx.session.buffer.is_empty() {
        if let Some(c) = key_to_char(key, false, ctx.session_state.caps_lock_enabled) {
            ctx.session.shift_used_as_modifier = true;
            if ctx.session.filter_mode != FilterMode::Global {
                ctx.session.filter_mode = FilterMode::Global;
            }
            ctx.session.handle_filter_char(c);

            if let Some(act) = lookup(ctx) {
                return act;
            }
            return Compositor::update_phantom_action(ctx);
        }
    }

    let current_profile = ctx
        .session_state
        .active_profiles
        .first()
        .cloned()
        .unwrap_or_default();
    if let Some(scheme) = ctx.engine.schemes.get(&current_profile) {
        let context = crate::scheme::SchemeContext {
            config: &ctx.config.master_config,
            tries: &std::collections::HashMap::new(),
            syllables: &ctx.syllables,
            _user_dict: &ctx.config.learned_words,
            active_profiles: &ctx.session_state.active_profiles,
            candidate_count: ctx.session.candidates.len(),
            _filter_mode: ctx.session.filter_mode.clone(),
            _aux_filter: &ctx.session.aux_filter,
        };
        let act_opt: Option<Action> =
            scheme.handle_special_key(key, &mut ctx.session.buffer, &context);
        if let Some(act) = act_opt {
            if act == Action::Consume {
                if perform_lookup {
                    if let Some(lookup_act) = lookup(ctx) {
                        return lookup_act;
                    }
                }
                return Compositor::update_phantom_action(ctx);
            }
            return act;
        }
    }

    if is_letter(key) {
        if ctx.session.filter_mode != FilterMode::None {
            if let Some(c) = key_to_char(key, shift_pressed, ctx.session_state.caps_lock_enabled) {
                ctx.session.handle_filter_char(c);
                if perform_lookup {
                    if let Some(act) = lookup(ctx) {
                        return act;
                    }
                }
                return Compositor::update_phantom_action(ctx);
            }
        }

        if !shift_pressed && ctx.config.enable_double_tap() {
            if let Some(last_k) = ctx.dispatcher.last_tap_key {
                if last_k == key {
                    if let Some(last_t) = ctx.dispatcher.last_tap_time {
                        if now.duration_since(last_t) <= ctx.config.double_tap_timeout() {
                            if let Some(c) =
                                key_to_char(key, false, ctx.session_state.caps_lock_enabled)
                            {
                                let lang = ctx
                                    .session_state
                                    .active_profiles
                                    .first()
                                    .cloned()
                                    .unwrap_or_default()
                                    .to_lowercase();

                                let mut replacement = None;
                                if let Some(layout) = ctx.config.layouts().get(&lang) {
                                    if let Some(action) = layout.mappings.get(&c.to_string()) {
                                        if let Some(dt) = &action.double_tap {
                                            replacement = Some(dt.clone());
                                        }
                                    }
                                }

                                if replacement.is_none() {
                                    replacement =
                                        ctx.config.double_taps().get(&c.to_string()).cloned();
                                }

                                if let Some(r) = replacement {
                                    if ctx.session.buffer.ends_with(c) {
                                        ctx.session.buffer.pop();
                                        ctx.session.buffer.push_str(&r);
                                        ctx.dispatcher.last_tap_key = None;
                                        ctx.dispatcher.last_tap_time = None;
                                        if perform_lookup {
                                            if let Some(act) = lookup(ctx) {
                                                return act;
                                            }
                                        }
                                        return Compositor::update_phantom_action(ctx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            ctx.dispatcher.last_tap_key = Some(key);
            ctx.dispatcher.last_tap_time = Some(now);
        } else {
            ctx.dispatcher.last_tap_key = None;
            ctx.dispatcher.last_tap_time = None;
        }

        if let Some(c) = key_to_char(key, shift_pressed, ctx.session_state.caps_lock_enabled) {
            if shift_pressed {
                ctx.session.shift_used_as_modifier = true;
            }
            ctx.session.push_char(c);
            if perform_lookup {
                if let Some(act) = lookup(ctx) {
                    return act;
                }
            }
            if should_block_invalid_input(ctx, &ctx.session.buffer.clone()) {
                return Action::Alert;
            }
            if let Some(act) = Compositor::check_auto_commit(ctx) {
                return act;
            }
            return Compositor::update_phantom_action(ctx);
        }
    } else {
        ctx.dispatcher.last_tap_key = None;
        ctx.dispatcher.last_tap_time = None;
    }

    if ctx.config.page_up_keys().contains(&key) && has_cand {
        return commands::execute_command(ctx, Command::PrevPage);
    }
    if ctx.config.page_down_keys().contains(&key) && has_cand {
        return commands::execute_command(ctx, Command::NextPage);
    }
    if ctx.config.prev_candidate_keys().contains(&key) && has_cand {
        return commands::execute_command(ctx, Command::PrevCandidate);
    }
    if ctx.config.next_candidate_keys().contains(&key) && has_cand {
        return commands::execute_command(ctx, Command::NextCandidate);
    }

    if key == VirtualKey::Semicolon && !shift_pressed {
        ctx.session.push_char(';');
        if perform_lookup {
            if let Some(act) = lookup(ctx) {
                return act;
            }
        }
        return Compositor::update_phantom_action(ctx);
    }

    match key {
        VirtualKey::Backspace => {
            if ctx.session.filter_mode != FilterMode::None {
                ctx.session.pop_filter();
                if perform_lookup {
                    if let Some(act) = lookup(ctx) {
                        return act;
                    }
                }
                return Compositor::update_phantom_action(ctx);
            }

            if ctx.session.buffer.is_empty() {
                ctx.session_state.commit_history.clear();
                return Action::PassThrough;
            }

            let old_phantom_len = ctx.session.phantom_text.chars().count();
            ctx.session.pop_char();

            if ctx.session.buffer.is_empty() {
                ctx.reset();
                if old_phantom_len > 0 {
                    Action::DeleteAndEmit {
                        delete: old_phantom_len,
                        insert: "".into(),
                    }
                } else {
                    Action::Consume
                }
            } else {
                if perform_lookup {
                    if let Some(act) = lookup(ctx) {
                        return act;
                    }
                }
                Compositor::update_phantom_action(ctx)
            }
        }

        VirtualKey::Home => {
            if shift_pressed {
                ctx.session.selected = 0;
                ctx.session.page = 0;
            } else {
                ctx.session.selected = ctx.session.page;
            }
            Action::Consume
        }
        VirtualKey::End => {
            if has_cand {
                if shift_pressed {
                    ctx.session.selected = ctx.session.candidates.len() - 1;
                    ctx.session.page =
                        (ctx.session.selected / ctx.config.page_size()) * ctx.config.page_size();
                } else {
                    ctx.session.selected = (ctx.session.page + ctx.config.page_size() - 1)
                        .min(ctx.session.candidates.len() - 1);
                }
            }
            Action::Consume
        }

        VirtualKey::Apostrophe if !shift_pressed => {
            ctx.session.buffer.push('\'');
            ctx.session.preview_selected_candidate = false;
            if perform_lookup {
                if let Some(act) = lookup(ctx) {
                    return act;
                }
            }
            Compositor::update_phantom_action(ctx)
        }

        VirtualKey::Slash if !ctx.session.buffer.is_empty() => {
            let mut new_buffer = ctx.session.buffer.clone();
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
                ctx.session.buffer = new_buffer;
                if perform_lookup {
                    if let Some(act) = lookup(ctx) {
                        return act;
                    }
                }
                return Compositor::update_phantom_action(ctx);
            }
            Action::PassThrough
        }

        _ if is_digit(key) => {
            let digit = key_to_digit(key).unwrap_or(0);
            if ctx.config.enable_number_selection()
                && ctx.config.commit_mode() == "single"
                && digit >= 1
                && digit <= ctx.config.page_size()
            {
                return commands::execute_command(ctx, Command::Select(digit as usize - 1));
            }
            let old_buffer = ctx.session.buffer.clone();
            ctx.session.push_char(
                key_to_char(key, false, ctx.session_state.caps_lock_enabled).unwrap_or('0'),
            );
            if perform_lookup {
                if let Some(act) = lookup(ctx) {
                    return act;
                }
            }

            if should_block_invalid_input(ctx, &old_buffer) {
                return Action::Alert;
            }
            if let Some(act) = Compositor::check_auto_commit(ctx) {
                return act;
            }
            return Compositor::update_phantom_action(ctx);
        }
        _ => {
            if get_punctuation_key(key, shift_pressed).is_some() {
                return handle_punctuation(ctx, key, shift_pressed);
            } else if let Some(c) =
                key_to_char(key, shift_pressed, ctx.session_state.caps_lock_enabled)
            {
                let old_buffer = ctx.session.buffer.clone();
                ctx.session.push_char(c);
                if perform_lookup {
                    if let Some(act) = lookup(ctx) {
                        return act;
                    }
                }
                if should_block_invalid_input(ctx, &old_buffer) {
                    return Action::Alert;
                }
                if let Some(act) = Compositor::check_auto_commit(ctx) {
                    return act;
                }
                return Compositor::update_phantom_action(ctx);
            } else {
                return Action::PassThrough;
            }
        }
    }
}

pub fn handle_punctuation(ctx: &mut EngineContext, key: VirtualKey, shift_pressed: bool) -> Action {
    use crate::processor::punctuation;
    punctuation::handle_punctuation(ctx, key, shift_pressed)
}
