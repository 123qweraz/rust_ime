use crate::engine::keys::VirtualKey;
use crate::engine::processor::utils::get_punctuation_key;
use crate::engine::processor::{Action, Processor};

fn resolve_layout_punctuation(
    processor: &Processor,
    lang: &str,
    key: VirtualKey,
    shift_pressed: bool,
) -> Option<String> {
    let layout = processor.config.layouts.get(lang)?;
    let shifted_key = get_punctuation_key(key, true);
    let base_key = get_punctuation_key(key, false);

    if shift_pressed {
        if let Some(k) = shifted_key {
            if let Some(action) = layout.mappings.get(k) {
                if !action.tap.is_empty() {
                    return Some(action.tap.clone());
                }
            }
        }
        if let Some(k) = base_key {
            if let Some(action) = layout.mappings.get(k) {
                if !action.shift.is_empty() {
                    return Some(action.shift.clone());
                }
            }
        }
    } else if let Some(k) = base_key {
        if let Some(action) = layout.mappings.get(k) {
            if !action.tap.is_empty() {
                return Some(action.tap.clone());
            }
        }
    }
    None
}

pub fn handle_punctuation(
    processor: &mut Processor,
    key: VirtualKey,
    shift_pressed: bool,
) -> Action {
    let punc_key_owned = get_punctuation_key(key, shift_pressed)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{:?}", key));
    let punc_key = punc_key_owned.as_str();
    let lang = processor
        .active_profiles
        .first()
        .cloned()
        .unwrap_or_else(|| "chinese".to_string())
        .to_lowercase();

    let zh_punc =
        if let Some(mapped) = resolve_layout_punctuation(processor, &lang, key, shift_pressed) {
            mapped
        } else if lang == "japanese" {
            match (punc_key, shift_pressed) {
                (".", false) => "。".to_string(),
                (",", false) => "、".to_string(),
                ("?", _) => "？".to_string(),
                ("!", _) => "！".to_string(),
                ("/", false) => "・".to_string(),
                ("[", false) => "「".to_string(),
                ("]", false) => "」".to_string(),
                ("-", false) => "ー".to_string(),
                ("-", true) => "＝".to_string(),
                _ => punc_key.to_string(),
            }
        } else {
            let zh_puncs = processor
                .config
                .punctuations
                .get(&lang)
                .and_then(|m| m.get(punc_key))
                .or_else(|| {
                    processor
                        .config
                        .punctuations
                        .get("chinese")
                        .and_then(|m| m.get(punc_key))
                });

            if let Some(entries) = zh_puncs {
                if punc_key == "\"" {
                    let p = if processor.session.quote_open {
                        entries.get(1).or(entries.first())
                    } else {
                        entries.first()
                    };
                    processor.session.quote_open = !processor.session.quote_open;
                    p.map(|e| e.char.clone())
                        .unwrap_or_else(|| punc_key.to_string())
                } else if punc_key == "'" {
                    let p = if processor.session.single_quote_open {
                        entries.get(1).or(entries.first())
                    } else {
                        entries.first()
                    };
                    processor.session.single_quote_open = !processor.session.single_quote_open;
                    p.map(|e| e.char.clone())
                        .unwrap_or_else(|| punc_key.to_string())
                } else {
                    entries
                        .first()
                        .map(|e| e.char.clone())
                        .unwrap_or_else(|| punc_key.to_string())
                }
            } else {
                punc_key.to_string()
            }
        };

    let mut commit_text = if !processor.session.joined_sentence.is_empty() {
        processor.session.joined_sentence.trim_end().to_string()
    } else if !processor.session.candidates.is_empty() {
        processor.session.candidates[0].text.trim_end().to_string()
    } else {
        processor.session.buffer.trim_end().to_string()
    };
    commit_text.push_str(&zh_punc);
    let del_len = processor.session.phantom_text.chars().count();
    processor.clear_composing();
    processor.commit_history.clear();
    Action::DeleteAndEmit {
        delete: del_len,
        insert: commit_text,
    }
}
