use crate::engine::processor::{Processor, ImeState};

pub struct Compositor;

impl Compositor {
    pub fn get_preedit(p: &Processor) -> String {
        if p.session.buffer.is_empty() || !p.chinese_enabled {
            return String::new();
        }

        let mut pinyin = if p.session.best_segmentation.is_empty() {
            p.session.buffer.clone()
        } else {
            // 尝试维持 buffer 的原始大小写
            let mut result = String::new();
            let mut current_pos = 0;
            let buffer_chars: Vec<char> = p.session.buffer.chars().collect();
            
            for (i, seg) in p.session.best_segmentation.iter().enumerate() {
                if i > 0 { result.push(' '); }
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
                    for uc in c.to_uppercase() { display_aux.push(uc); }
                } else {
                    for lc in c.to_lowercase() { display_aux.push(lc); }
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
            PhantomType::Pinyin => p.session.buffer.clone(),
            PhantomType::Hanzi => {
                if p.session.preview_selected_candidate && !p.session.candidates.is_empty() {
                    p.session.candidates[p.session.selected.min(p.session.candidates.len() - 1)].text.to_string()
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
