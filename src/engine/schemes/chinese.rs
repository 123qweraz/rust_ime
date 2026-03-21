use crate::engine::processor::strip_tones;
use crate::engine::scheme::{InputScheme, SchemeCandidate, SchemeContext};

pub struct ChineseScheme;

#[derive(Debug, Clone)]
struct ParsedPart {
    pinyin: String,
    stroke_aux: Option<String>,
    english_aux: Option<String>,
}

impl ChineseScheme {
    pub fn new() -> Self {
        Self
    }

    fn parse_buffer(&self, buffer: &str) -> Vec<ParsedPart> {
        let buffer_normalized = strip_tones(buffer);
        let parts: Vec<&str> = buffer_normalized
            .split(' ')
            .filter(|s| !s.is_empty())
            .collect();
        let mut result = Vec::new();

        for part in parts {
            let mut stroke_aux = None;
            let mut english_aux = None;

            let pinyin_end = part
                .char_indices()
                .find(|(i, c)| {
                    *c == ';' || c.is_ascii_digit() || (*i > 0 && c.is_ascii_uppercase())
                })
                .map(|(i, _)| i)
                .unwrap_or(part.len());

            let pinyin = part[..pinyin_end].to_string();
            let mut rest = &part[pinyin_end..];

            if rest.starts_with(';') {
                rest = &rest[1..];
                let stroke_end = rest
                    .find(|c: char| c.is_ascii_digit() || c.is_ascii_uppercase())
                    .unwrap_or(rest.len());
                let s = &rest[..stroke_end];
                if !s.is_empty() {
                    stroke_aux = Some(s.to_string());
                }
                rest = &rest[stroke_end..];
            }

            if !rest.is_empty() && rest.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                let english_end = rest
                    .find(|c: char| c.is_ascii_digit())
                    .unwrap_or(rest.len());
                let e = &rest[..english_end];
                if !e.is_empty() {
                    english_aux = Some(e.to_string());
                }
            }

            result.push(ParsedPart {
                pinyin,
                stroke_aux,
                english_aux,
            });
        }
        result
    }

    fn get_fuzzy_variants(&self, pinyin: &str, context: &SchemeContext) -> Vec<String> {
        let mut new_variants = std::collections::HashSet::new();
        new_variants.insert(pinyin.to_string());

        if !context.config.input.enable_fuzzy_pinyin {
            return vec![pinyin.to_string()];
        }

        let cfg = &context.config.input.fuzzy_config;

        // 声母转换
        let initial_list: Vec<String> = new_variants.iter().cloned().collect();
        for v in initial_list {
            if cfg.z_zh {
                if v.starts_with("zh") {
                    new_variants.insert(v.replacen("zh", "z", 1));
                } else if v.starts_with("z") {
                    new_variants.insert(v.replacen("z", "zh", 1));
                }
            }
            if cfg.c_ch {
                if v.starts_with("ch") {
                    new_variants.insert(v.replacen("ch", "c", 1));
                } else if v.starts_with("c") {
                    new_variants.insert(v.replacen("c", "ch", 1));
                }
            }
            if cfg.s_sh {
                if v.starts_with("sh") {
                    new_variants.insert(v.replacen("sh", "s", 1));
                } else if v.starts_with("s") {
                    new_variants.insert(v.replacen("s", "sh", 1));
                }
            }
            if cfg.n_l {
                if v.starts_with('n') {
                    new_variants.insert(v.replacen('n', "l", 1));
                } else if v.starts_with('l') {
                    new_variants.insert(v.replacen('l', "n", 1));
                }
            }
            if cfg.r_l {
                if v.starts_with('r') {
                    new_variants.insert(v.replacen('r', "l", 1));
                } else if v.starts_with('l') {
                    new_variants.insert(v.replacen('l', "r", 1));
                }
            }
            if cfg.f_h {
                if v.starts_with('f') {
                    new_variants.insert(v.replacen('f', "h", 1));
                } else if v.starts_with('h') {
                    new_variants.insert(v.replacen('h', "f", 1));
                }
            }
        }

        // 韵母转换
        let current_list: Vec<String> = new_variants.iter().cloned().collect();
        for v in current_list {
            if cfg.an_ang {
                if v.ends_with("ang") {
                    new_variants.insert(v.replace("ang", "an"));
                } else if v.ends_with("an") {
                    new_variants.insert(v.replace("an", "ang"));
                }
            }
            if cfg.en_eng {
                if v.ends_with("eng") {
                    new_variants.insert(v.replace("eng", "en"));
                } else if v.ends_with("en") {
                    new_variants.insert(v.replace("en", "eng"));
                }
            }
            if cfg.in_ing {
                if v.ends_with("ing") {
                    new_variants.insert(v.replace("ing", "in"));
                } else if v.ends_with("in") {
                    new_variants.insert(v.replace("in", "ing"));
                }
            }
            if cfg.ian_iang {
                if v.ends_with("iang") {
                    new_variants.insert(v.replace("iang", "ian"));
                } else if v.ends_with("ian") {
                    new_variants.insert(v.replace("ian", "iang"));
                }
            }
            if cfg.uan_uang {
                if v.ends_with("uang") {
                    new_variants.insert(v.replace("uang", "uan"));
                } else if v.ends_with("uan") {
                    new_variants.insert(v.replace("uan", "uang"));
                }
            }
            if cfg.u_v {
                if v.contains('u') {
                    new_variants.insert(v.replace('u', "v"));
                } else if v.contains('v') {
                    new_variants.insert(v.replace('v', "u"));
                }
            }
        }

        // 自定义映射
        let current_list: Vec<String> = new_variants.iter().cloned().collect();
        for v in current_list {
            for (from, to) in &cfg.custom_mappings {
                if v.contains(from) {
                    new_variants.insert(v.replace(from, to));
                }
            }
        }

        new_variants.into_iter().collect()
    }

    fn segment_buffer(&self, input: &str, context: &SchemeContext) -> Vec<String> {
        let mut segments = Vec::new();
        let mut remaining = input.to_lowercase();

        while !remaining.is_empty() {
            let mut matched = false;
            for len in (1..=6).rev() {
                if len <= remaining.len() {
                    let part = &remaining[..len];
                    if context.syllables.contains(part) {
                        segments.push(part.to_string());
                        remaining = remaining[len..].to_string();
                        matched = true;
                        break;
                    }
                }
            }
            if matched {
                continue;
            }

            let c = remaining.chars().next().unwrap_or('\0');
            let is_initial = "bpmfdtnlgkhjqxzcsryw".contains(c);
            if is_initial {
                let initial_len = if remaining.starts_with("zh")
                    || remaining.starts_with("ch")
                    || remaining.starts_with("sh")
                {
                    2
                } else {
                    1
                };
                segments.push(remaining[..initial_len].to_string());
                remaining = remaining[initial_len..].to_string();
            } else {
                segments.push(remaining[..1].to_string());
                remaining = remaining[1..].to_string();
            }
        }
        segments
    }
}

impl InputScheme for ChineseScheme {
    fn name(&self) -> &str {
        "chinese"
    }

    fn lookup(&self, query: &str, context: &SchemeContext) -> Vec<SchemeCandidate> {
        let raw_parsed = self.parse_buffer(query);
        let mut final_results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 策略 1: 全量/简拼/前缀匹配
        let mut smart_segments = Vec::new();
        if !query.contains(' ') && !query.contains('\'') {
            let pinyin_only: String = raw_parsed.iter().map(|p| p.pinyin.clone()).collect();
            smart_segments = self.segment_buffer(&pinyin_only, context);
        }

        // 原始切分检索
        let mut last_matches_raw = Vec::new();
        for (i, part) in raw_parsed.iter().enumerate() {
            let mut matches = Vec::new();
            let pinyin_variants = self.get_fuzzy_variants(&part.pinyin, context);

            for profile in context.active_profiles {
                if let Some(d) = context.tries.get(profile) {
                    for py in &pinyin_variants {
                        if let Some(m) = d.get_all_exact(py) {
                            for tr in m {
                                matches.push((
                                    tr.word.to_string(),
                                    tr.trad.to_string(),
                                    tr.tone.to_string(),
                                    tr.en.to_string(),
                                    tr.stroke_aux.to_string(),
                                    tr.weight,
                                    3,
                                ));
                            }
                        }
                        if context.config.input.enable_prefix_matching && !py.is_empty() {
                            let limit = if part.stroke_aux.is_some() || part.english_aux.is_some() {
                                50
                            } else if py.len() > 3 {
                                5
                            } else {
                                20
                            };
                            let m = d.search_bfs(py, limit);
                            for tr in m {
                                matches.push((
                                    tr.word.to_string(),
                                    tr.trad.to_string(),
                                    tr.tone.to_string(),
                                    tr.en.to_string(),
                                    tr.stroke_aux.to_string(),
                                    tr.weight,
                                    1,
                                ));
                            }
                        }
                    }
                }
            }
            if i == raw_parsed.len() - 1 {
                last_matches_raw = matches;
            }
        }

        // 辅码过滤
        for m in last_matches_raw {
            let last_part = raw_parsed.last();
            if let Some(aux) = last_part.and_then(|p| p.stroke_aux.as_ref()) {
                let aux_lower = aux.to_lowercase();
                let stroke_aux_lower = m.4.to_lowercase();
                if !stroke_aux_lower.starts_with(&aux_lower) {
                    continue;
                }
            }
            if let Some(aux) = last_part.and_then(|p| p.english_aux.as_ref()) {
                let aux_lower = aux.to_lowercase();
                if !m
                    .3
                    .to_lowercase()
                    .split(',')
                    .any(|part: &str| part.trim().starts_with(&aux_lower))
                {
                    continue;
                }
            }

            if seen.insert(m.0.clone()) {
                let mut cand = SchemeCandidate::new(m.0, m.5);
                cand.traditional = m.1;
                cand.tone = m.2;
                cand.english = m.3;
                cand.stroke_aux = m.4;
                cand.match_level = m.6;
                final_results.push(cand);
            }
        }

        // 策略 2: 简拼检索
        if context.config.input.enable_abbreviation_matching
            && !smart_segments.is_empty()
            && smart_segments.len() > 1
        {
            let first_seg_variants = self.get_fuzzy_variants(&smart_segments[0], context);
            for v1 in &first_seg_variants {
                let mut modified_segments = smart_segments.clone();
                modified_segments[0] = v1.clone();
                if let Some(d) = context.tries.get("chinese") {
                    let m = d.search_abbreviation(&modified_segments, context.syllables, 500);
                    for tr in m {
                        let last_part = raw_parsed.last();
                        if let Some(aux) = last_part.and_then(|p| p.stroke_aux.as_ref()) {
                            let aux_lower = aux.to_lowercase();
                            if !tr.stroke_aux.to_lowercase().starts_with(&aux_lower) {
                                continue;
                            }
                        }
                        if let Some(aux) = last_part.and_then(|p| p.english_aux.as_ref()) {
                            let aux_lower = aux.to_lowercase();
                            if !tr
                                .en
                                .to_lowercase()
                                .split(',')
                                .any(|part: &str| part.trim().starts_with(&aux_lower))
                            {
                                continue;
                            }
                        }
                        if seen.insert(tr.word.to_string()) {
                            let mut cand = SchemeCandidate::new(tr.word.to_string(), tr.weight);
                            cand.traditional = tr.trad.to_string();
                            cand.tone = tr.tone.to_string();
                            cand.english = tr.en.to_string();
                            cand.stroke_aux = tr.stroke_aux.to_string();
                            cand.match_level = 2;
                            final_results.push(cand);
                        }
                    }
                }
            }
        }
        final_results
    }

    fn post_process(
        &self,
        query: &str,
        candidates: &mut Vec<SchemeCandidate>,
        context: &SchemeContext,
    ) {
        let raw_parsed = self.parse_buffer(query);
        let pinyin_only: String = raw_parsed.iter().map(|p| p.pinyin.clone()).collect();
        let smart_segments = self.segment_buffer(&pinyin_only, context);
        let input_syllables = if smart_segments.is_empty() {
            raw_parsed.len()
        } else {
            smart_segments.len()
        };

        candidates.sort_by(|a, b| {
            let get_score = |m: &SchemeCandidate| -> i64 {
                let level = m.match_level as i64;
                let weight = m.weight as i64;
                let char_count = m.text.chars().count() as i64;
                let mut score = if level == 3 {
                    40_000_000
                } else {
                    level * 10_000_000
                };
                if level == 2 && char_count == input_syllables as i64 {
                    score += 10_000_000;
                }
                score += weight;
                let len_diff = (char_count - input_syllables as i64).max(0);
                score -= len_diff * (if level == 2 { 10000 } else { 1000 });
                score
            };
            get_score(b).cmp(&get_score(a))
        });
    }
}
