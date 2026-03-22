use crate::processor::commands::commit_candidate;
use crate::processor::{Action, FilterMode, ImeState};
use crate::EngineContext;

pub struct Compositor;

impl Compositor {
    pub fn get_preedit(ctx: &EngineContext) -> String {
        if ctx.session.buffer.is_empty() || !ctx.session_state.chinese_enabled {
            return String::new();
        }

        let is_stroke = ctx
            .session_state
            .active_profiles
            .iter()
            .any(|profile| profile == "stroke");

        let mut pinyin = if is_stroke {
            ctx.session.buffer.clone()
        } else if ctx.session.best_segmentation.is_empty() {
            ctx.session.buffer.clone()
        } else {
            let mut result = String::new();
            let mut current_pos = 0;
            let buffer_chars: Vec<char> = ctx.session.buffer.chars().collect();

            for (i, seg) in ctx.session.best_segmentation.iter().enumerate() {
                if i > 0 {
                    result.push(' ');
                }
                let seg_len = seg.chars().count();
                for j in 0..seg_len {
                    if current_pos + j < buffer_chars.len() {
                        result.push(buffer_chars[current_pos + j]);
                    }
                }
                current_pos += seg_len;
            }
            if current_pos < buffer_chars.len() {
                result.push_str(&ctx.session.buffer[current_pos..]);
            }
            result
        };

        if ctx.session.nav_mode {
            pinyin.push_str(" [H:左 J:下 K:上 L:右]");
        }

        if !ctx.session.aux_filter.is_empty() {
            let mut display_aux = String::new();
            for (i, c) in ctx.session.aux_filter.chars().enumerate() {
                if i == 0 {
                    for uc in c.to_uppercase() {
                        display_aux.push(uc);
                    }
                } else {
                    for lc in c.to_lowercase() {
                        display_aux.push(lc);
                    }
                }
            }
            pinyin.push_str(&display_aux);
        }

        pinyin
    }

    pub fn get_phantom_text(ctx: &mut EngineContext) -> String {
        use rust_ime_core::config::PhantomType;
        if ctx.session.state == ImeState::Idle || ctx.config.phantom_type() == PhantomType::None {
            return String::new();
        }

        if ctx.session.switch_mode {
            return "[方案切换]".to_string();
        }

        match ctx.config.phantom_type() {
            PhantomType::Pinyin => {
                if ctx
                    .session_state
                    .active_profiles
                    .contains(&"stroke".to_string())
                    && ctx.session.buffer.chars().any(|c| c.is_ascii_digit())
                {
                    let converted = convert_stroke_digits_to_letters(&ctx.session.buffer);
                    if !converted.is_empty() {
                        return converted;
                    }
                }
                ctx.session.buffer.clone()
            }
            PhantomType::Hanzi => {
                if ctx.session.preview_selected_candidate && !ctx.session.candidates.is_empty() {
                    ctx.session.candidates
                        [ctx.session.selected.min(ctx.session.candidates.len() - 1)]
                    .text
                    .to_string()
                } else if !ctx.session.joined_sentence.is_empty() {
                    ctx.session.joined_sentence.clone()
                } else if !ctx.session.candidates.is_empty() {
                    ctx.session.candidates[0].text.to_string()
                } else {
                    ctx.session.buffer.clone()
                }
            }
            _ => String::new(),
        }
    }

    pub fn update_phantom_action(ctx: &mut EngineContext) -> Action {
        if ctx.config.phantom_type() == rust_ime_core::config::PhantomType::None {
            return Action::Consume;
        }
        let target = Self::get_phantom_text(ctx);
        if target == ctx.session.phantom_text {
            return Action::Consume;
        }
        let old_phantom = ctx.session.phantom_text.clone();
        let old_chars: Vec<char> = old_phantom.chars().collect();
        let target_chars: Vec<char> = target.chars().collect();
        let mut common_prefix_len = 0;
        for (c1, c2) in old_chars.iter().zip(target_chars.iter()) {
            if c1 == c2 {
                common_prefix_len += 1;
            } else {
                break;
            }
        }
        let delete_count = old_chars.len() - common_prefix_len;
        let insert_text: String = target_chars[common_prefix_len..].iter().collect();
        ctx.session.phantom_text = target;
        if delete_count == 0 && insert_text.is_empty() {
            Action::Consume
        } else if delete_count == 0 {
            Action::Emit(insert_text)
        } else {
            Action::DeleteAndEmit {
                delete: delete_count,
                insert: insert_text,
            }
        }
    }

    pub fn check_auto_commit(ctx: &mut EngineContext) -> Option<Action> {
        if ctx.session.candidates.is_empty() || !ctx.session.has_dict_match {
            return None;
        }

        let raw_input = &ctx.session.buffer;

        if ctx.config.auto_commit_stroke() && ctx.session_state.is_stroke_mode() {
            if !ctx.session.candidates.is_empty() && ctx.session.candidates[0].match_level == 3 {
                let is_unique_exact =
                    ctx.session.candidates.len() == 1 || ctx.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = ctx.session.candidates[0].text.clone();
                    return Some(commit_candidate(ctx, word, 0));
                }
            }
        }

        if raw_input.contains(';') && !ctx.session.candidates.is_empty() {
            if ctx.session.candidates[0].match_level == 3 {
                let is_unique_exact =
                    ctx.session.candidates.len() == 1 || ctx.session.candidates[1].match_level != 3;
                if is_unique_exact {
                    let word = ctx.session.candidates[0].text.clone();
                    return Some(commit_candidate(ctx, word, 0));
                }
            }
        }

        if !ctx.config.auto_commit_unique_full_match() || ctx.session.candidates.len() != 1 {
            return None;
        }

        let has_longer = ctx
            .session_state
            .active_profiles
            .iter()
            .any(|p| ctx.engine.has_longer_match(p, raw_input));
        if !has_longer {
            let word = ctx.session.candidates[0].text.clone();
            return Some(commit_candidate(ctx, word, 0));
        }
        None
    }
}

pub fn start_global_filter(ctx: &mut EngineContext) {
    if ctx.session.state == ImeState::Idle {
        return;
    }
    if ctx.session.filter_mode != FilterMode::Global {
        ctx.session.filter_mode = FilterMode::Global;
        ctx.session.aux_filter.clear();
    }
}

pub fn should_block_invalid_input(ctx: &mut EngineContext, old_buffer: &str) -> bool {
    use rust_ime_core::config::AntiTypoMode;

    if ctx.session.has_dict_match {
        ctx.session.last_blocked_buffer.clear();
        return false;
    }
    match ctx.config.anti_typo_mode() {
        AntiTypoMode::None => false,
        AntiTypoMode::Strict => {
            ctx.session.buffer = old_buffer.to_string();
            let _ = crate::pipeline::lookup(ctx);
            true
        }
        AntiTypoMode::Smart => {
            if !ctx.session.last_blocked_buffer.is_empty()
                && ctx.session.buffer == ctx.session.last_blocked_buffer
            {
                ctx.session.last_blocked_buffer.clear();
                false
            } else {
                ctx.session.last_blocked_buffer = ctx.session.buffer.clone();
                ctx.session.buffer = old_buffer.to_string();
                let _ = crate::pipeline::lookup(ctx);
                true
            }
        }
    }
}

fn convert_stroke_digits_to_letters(s: &str) -> String {
    let mut res = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 1 < chars.len() {
            let pair = format!("{}{}", chars[i], chars[i + 1]);
            let code = match pair.as_str() {
                "11" => 'g',
                "12" => 'f',
                "13" => 'd',
                "14" => 's',
                "15" => 'a',
                "21" => 'h',
                "22" => 'j',
                "23" => 'k',
                "24" => 'l',
                "25" => 'm',
                "31" => 't',
                "32" => 'r',
                "33" => 'e',
                "34" => 'w',
                "35" => 'q',
                "41" => 'y',
                "42" => 'u',
                "43" => 'i',
                "44" => 'o',
                "45" => 'p',
                "51" => 'n',
                "52" => 'b',
                "53" => 'v',
                "54" => 'c',
                "55" => 'x',
                _ => ' ',
            };
            if code != ' ' {
                res.push(code);
                i += 2;
                continue;
            }
        }
        let code = match chars[i] {
            '1' => 'g',
            '2' => 'h',
            '3' => 't',
            '4' => 'y',
            '5' => 'n',
            c if c.is_ascii_lowercase() => c,
            _ => ' ',
        };
        if code != ' ' {
            res.push(code);
        }
        i += 1;
    }
    res
}
