use crate::engine::processor::{ImeState, Processor};

pub struct Compositor;

impl Compositor {
    pub fn get_preedit(p: &Processor) -> String {
        if p.session.buffer.is_empty() || !p.session_state.chinese_enabled {
            return String::new();
        }

        // 笔画输入法不需要拼音分词，直接使用buffer
        let is_stroke = p
            .session_state
            .active_profiles
            .iter()
            .any(|profile| profile == "stroke");

        let mut pinyin = if is_stroke {
            // 笔画输入法：不使用拼音分词
            p.session.buffer.clone()
        } else if p.session.best_segmentation.is_empty() {
            p.session.buffer.clone()
        } else {
            // 拼音输入法：使用拼音分词
            let mut result = String::new();
            let mut current_pos = 0;
            let buffer_chars: Vec<char> = p.session.buffer.chars().collect();

            for (i, seg) in p.session.best_segmentation.iter().enumerate() {
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
            // 补齐剩余部分 (如果有)
            if current_pos < buffer_chars.len() {
                result.push_str(&p.session.buffer[current_pos..]);
            }
            result
        };

        if p.session.nav_mode {
            pinyin.push_str(" [H:左 J:下 K:上 L:右]");
        }

        if !p.session.aux_filter.is_empty() {
            let mut display_aux = String::new();
            for (i, c) in p.session.aux_filter.chars().enumerate() {
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

    pub fn get_phantom_text(p: &Processor) -> String {
        use crate::config::PhantomType;
        if p.session.state == ImeState::Idle || p.config.phantom_type == PhantomType::None {
            return String::new();
        }

        if p.session.switch_mode {
            return "[方案切换]".to_string();
        }

        match p.config.phantom_type {
            PhantomType::Pinyin => {
                // 对于笔画输入法，显示转换后的字母编码而不是原始数字
                // 笔画输入法的 buffer 包含数字（如 "12345"），需要转换为字母编码
                if p.session_state
                    .active_profiles
                    .contains(&"stroke".to_string())
                    && p.session.buffer.chars().any(|c| c.is_ascii_digit())
                {
                    // 如果 buffer 包含数字，说明是笔画输入，需要转换
                    // 这里我们使用一个简单的转换逻辑，因为无法直接调用 scheme 的方法
                    let converted = convert_stroke_digits_to_letters(&p.session.buffer);
                    if !converted.is_empty() {
                        return converted;
                    }
                }
                p.session.buffer.clone()
            }
            PhantomType::Hanzi => {
                if p.session.preview_selected_candidate && !p.session.candidates.is_empty() {
                    p.session.candidates[p.session.selected.min(p.session.candidates.len() - 1)]
                        .text
                        .to_string()
                } else if !p.session.joined_sentence.is_empty() {
                    p.session.joined_sentence.clone()
                } else if !p.session.candidates.is_empty() {
                    p.session.candidates[0].text.to_string()
                } else {
                    p.session.buffer.clone()
                }
            }
            _ => String::new(),
        }
    }
}

/// 将笔画数字转换为字母编码（与 StrokeScheme::encode_stroke 相同的逻辑）
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
