use crate::keys::VirtualKey;
use crate::processor::utils::*;
use crate::processor::{inject_text, Action, ImeState};
use crate::{compositor, EngineContext};
use std::time::Instant;

pub fn process_modifiers(
    ctx: &mut EngineContext,
    key: VirtualKey,
    is_press: bool,
    is_release: bool,
) -> Option<Action> {
    if is_press && key == VirtualKey::Shift {
        ctx.session.shift_used_as_modifier = false;
    }

    if is_release {
        if key == VirtualKey::Shift {
            if !ctx.session.buffer.is_empty() {
                if !ctx.session.shift_used_as_modifier {
                    compositor::start_global_filter(ctx);
                }
                ctx.session.shift_used_as_modifier = false;
                return Some(Action::Consume);
            }
            ctx.session.shift_used_as_modifier = false;
        }

        if matches!(
            key,
            VirtualKey::Control | VirtualKey::Alt | VirtualKey::Shift | VirtualKey::CapsLock
        ) {
            return Some(Action::PassThrough);
        }

        if ctx.session.buffer.is_empty() {
            return Some(Action::PassThrough);
        }
        return Some(Action::Consume);
    }

    if key == VirtualKey::CapsLock && is_press {
        return None;
    }

    None
}

pub fn process_intent(
    ctx: &mut EngineContext,
    key: VirtualKey,
    val: i32,
    shift_pressed: bool,
    now: Instant,
) -> Option<Action> {
    let is_repeat = val == 2;
    let is_release = val == 0;

    if ((ctx.config.enable_long_press() && is_letter(key))
        || (ctx.config.enable_punctuation_long_press()
            && get_punctuation_key(key, shift_pressed).is_some()))
        && !shift_pressed
    {
        if val == 1 {
            ctx.dispatcher.key_press_info = Some((key, now));
            ctx.dispatcher.long_press_triggered = false;
        } else if is_repeat {
            if !ctx.dispatcher.long_press_triggered {
                if let Some((press_key, press_time)) = ctx.dispatcher.key_press_info {
                    if press_key == key
                        && now.duration_since(press_time) >= ctx.config.long_press_timeout()
                    {
                        if is_letter(key) {
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
                                        replacement = action.long_press.clone();
                                    }
                                }

                                if replacement.is_none() {
                                    replacement = ctx
                                        .config
                                        .long_press_mappings()
                                        .get(&c.to_string())
                                        .cloned();
                                }

                                if let Some(r) = replacement {
                                    ctx.dispatcher.long_press_triggered = true;
                                    if !ctx.session.buffer.is_empty() {
                                        if let Some(last_char) = ctx.session.buffer.chars().last() {
                                            if last_char.to_string() == c.to_string() {
                                                ctx.session.buffer.pop();
                                            }
                                        }
                                    }
                                    return Some(inject_text(ctx, &r));
                                }
                            }
                        } else if let Some(p_key) = get_punctuation_key(key, false) {
                            let lang = ctx
                                .session_state
                                .active_profiles
                                .first()
                                .cloned()
                                .unwrap_or_default()
                                .to_lowercase();

                            let mut replacement = None;
                            if let Some(layout) = ctx.config.layouts().get(&lang) {
                                if let Some(action) = layout.mappings.get(p_key) {
                                    replacement = action.long_press.clone();
                                }
                            }

                            if replacement.is_none() {
                                replacement = ctx
                                    .config
                                    .punctuation_long_press_mappings()
                                    .get(p_key)
                                    .cloned();
                            }

                            if let Some(r) = replacement {
                                ctx.dispatcher.long_press_triggered = true;
                                let mut commit_text = if !ctx.session.joined_sentence.is_empty() {
                                    ctx.session.joined_sentence.trim_end().to_string()
                                } else if !ctx.session.candidates.is_empty() {
                                    ctx.session.candidates[0].text.trim_end().to_string()
                                } else {
                                    ctx.session.buffer.trim_end().to_string()
                                };
                                commit_text.push_str(&r);
                                let del_len = ctx.session.phantom_text.chars().count();
                                ctx.session.clear_composing();
                                ctx.session_state.commit_history.clear();
                                return Some(Action::DeleteAndEmit {
                                    delete: del_len,
                                    insert: commit_text,
                                });
                            }
                        }
                    }
                }
            }
            return Some(Action::Consume);
        } else if is_release {
            ctx.dispatcher.key_press_info = None;
            if ctx.dispatcher.long_press_triggered {
                return Some(Action::Consume);
            }
        }
    }
    None
}

pub fn process_switch_mode(
    ctx: &mut EngineContext,
    key: VirtualKey,
    is_press: bool,
    _is_release: bool,
) -> Option<Action> {
    if !ctx.session.switch_mode {
        return None;
    }

    if is_press {
        match key {
            VirtualKey::Esc | VirtualKey::Space | VirtualKey::Enter => {
                ctx.session.switch_mode = false;
                return Some(Action::Notify("快捷切换".into(), "已退出".into()));
            }
            VirtualKey::E => {
                ctx.session.switch_mode = false;
                if let Some((pinyin, word)) = ctx.session_state.commit_history.pop() {
                    let del_count = word.chars().count();
                    ctx.session.buffer = pinyin;
                    ctx.session.state = ImeState::Composing;
                    let _ = crate::pipeline::lookup(ctx);
                    return Some(Action::DeleteAndEmit {
                        delete: del_count,
                        insert: "".into(),
                    });
                }
                return Some(Action::Consume);
            }
            VirtualKey::Z => {
                ctx.session.switch_mode = false;
                let enabled = &ctx.config.master_config.input.enabled_profiles;
                if enabled.contains(&"english".to_string())
                    && ctx.engine.trie_paths.contains_key("english")
                {
                    ctx.session_state.active_profiles = vec!["english".to_string()];
                    ctx.reset();
                    return Some(Action::Notify("英".into(), "已切换至英语方案".into()));
                }
                return Some(Action::Consume);
            }
            _ if is_letter(key) => {
                let k = key_to_char(key, false, ctx.session_state.caps_lock_enabled)
                    .unwrap_or(' ')
                    .to_string();
                let mut target_profile = None;
                for (trigger_key, profile_name) in &ctx.config.profile_keys() {
                    if trigger_key == &k {
                        target_profile = Some(profile_name.clone());
                        break;
                    }
                }

                if let Some(p_str) = target_profile {
                    let enabled = &ctx.config.master_config.input.enabled_profiles;
                    let profiles: Vec<String> = p_str
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| {
                            !s.is_empty()
                                && ctx.engine.trie_paths.contains_key(s)
                                && enabled.contains(&s.to_string())
                        })
                        .collect();
                    if !profiles.is_empty() {
                        ctx.session_state.active_profiles = profiles;
                        let display = get_current_profile_display(&ctx);
                        let short_display = get_short_display(&ctx);
                        let _ = crate::pipeline::lookup(ctx);
                        ctx.session.switch_mode = false;
                        return Some(Action::Notify(short_display, format!("方案: {}", display)));
                    } else {
                        ctx.session.switch_mode = false;
                        return Some(Action::Notify(
                            "❌".into(),
                            format!("错误: 方案 [{}] 的词库未加载", p_str),
                        ));
                    }
                }
            }
            _ => {}
        }
        return Some(Action::Consume);
    }

    Some(Action::Consume)
}

fn get_current_profile_display(ctx: &EngineContext) -> String {
    if ctx.session_state.active_profiles.is_empty() {
        return "None".to_string();
    }
    if ctx.session_state.active_profiles.len() == 1 {
        return ctx.session_state.active_profiles[0].clone();
    }
    "Mixed".to_string()
}

fn get_short_display(ctx: &EngineContext) -> String {
    let display = get_current_profile_display(ctx);
    match display.to_lowercase().as_str() {
        "chinese" => "中".to_string(),
        "english" => "英".to_string(),
        "japanese" => "日".to_string(),
        "stroke" => "笔".to_string(),
        "mixed" => "混".to_string(),
        _ => display
            .chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string()),
    }
}
