use crate::engine::config_manager::UserDictData;
use crate::engine::trie::TrieResult;
use crate::engine::Trie;
use crate::Config;
use arc_swap::ArcSwap;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// 候选项
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub text: Arc<str>,
    pub simplified: Arc<str>,
    pub traditional: Arc<str>,
    pub hint: Arc<str>,
    pub source: Arc<str>, // 来源：如 "User", "Table", "Script"
    pub weight: f64,
    pub match_level: u8, // 0: unknown, 1: prefix, 2: abbreviation/wildcard, 3: exact
}

/* 核心接口定义 */

pub trait Segmentor: Send + Sync {
    fn segment(&self, input: &str, syllables: &HashSet<String>) -> Vec<String>;
}

pub trait Translator: Send + Sync {
    fn translate(
        &self,
        input: &str,
        segments: &[String],
        config: &Config,
        limit: usize,
    ) -> Vec<Candidate>;
}

pub trait Filter: Send + Sync {
    fn filter(
        &self,
        input: &str,
        candidates: Vec<Candidate>,
        config: &Config,
        context: Option<&str>,
    ) -> Vec<Candidate>;
}

/* 具体实现 */

/// 默认切分器实现 (Max Match)
pub struct DefaultSegmentor;
impl Segmentor for DefaultSegmentor {
    fn segment(&self, input: &str, syllables: &HashSet<String>) -> Vec<String> {
        let mut segments = Vec::new();
        let input_lower = input.to_lowercase();
        let mut remaining = input_lower.as_str();

        while !remaining.is_empty() {
            let mut matched = false;
            let max_len = 12.min(remaining.len());
            for len in (1..=max_len).rev() {
                if remaining.is_char_boundary(len) {
                    let part = &remaining[..len];
                    if syllables.contains(part) {
                        segments.push(part.to_string());
                        remaining = &remaining[len..];
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                if let Some(first_char) = remaining.chars().next() {
                    segments.push(first_char.to_string());
                    remaining = &remaining[first_char.len_utf8()..];
                } else {
                    break;
                }
            }
        }
        segments
    }
}

/// 系统词库翻译器
pub struct TableTranslator {
    pub trie: Arc<Trie>,
    pub syllables: Arc<HashSet<String>>,
    pub enable_abbreviation: bool,
}
impl Translator for TableTranslator {
    fn translate(
        &self,
        _input: &str,
        segments: &[String],
        config: &Config,
        limit: usize,
    ) -> Vec<Candidate> {
        if segments.is_empty() {
            return vec![];
        }
        let query = segments.join("");
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        let internal_limit = limit.max(500);

        let build_hint = |tr: &TrieResult| -> Arc<str> {
            let mut hint = String::new();
            if config.appearance.show_english_aux && !tr.en.is_empty() {
                hint.push_str(tr.en);
            }
            if config.appearance.show_stroke_aux && !tr.stroke_aux.is_empty() {
                if !hint.is_empty() {
                    hint.push(' ');
                }
                hint.push_str(tr.stroke_aux);
            }
            if hint.is_empty() {
                Arc::from(tr.tone)
            } else {
                Arc::from(hint.as_str())
            }
        };

        // 1. 尝试全拼精确匹配
        if let Some(exact_results) = self.trie.get_all_exact(&query) {
            for tr in exact_results {
                if seen.insert(tr.word) {
                    candidates.push(Candidate {
                        simplified: Arc::from(tr.word),
                        traditional: if tr.trad.is_empty() {
                            Arc::from(tr.word)
                        } else {
                            Arc::from(tr.trad)
                        },
                        text: Arc::from(tr.word),
                        hint: build_hint(&tr),
                        source: Arc::from("Table (Exact)"),
                        weight: tr.weight as f64 + config.input.ranking.exact_match_bonus,
                        match_level: 3,
                    });
                }
            }
        }

        let is_abbreviation =
            self.enable_abbreviation && segments.len() > 1 && segments.iter().any(|s| s.len() == 1);

        if is_abbreviation && config.input.enable_abbreviation_matching {
            let abbr_results =
                self.trie
                    .search_abbreviation(segments, &self.syllables, internal_limit);
            for ar in abbr_results {
                if seen.insert(ar.word) {
                    let adjusted_weight = if ar.weight > 8000 {
                        (ar.weight as f64) - 10.0
                    } else if ar.weight > 5000 {
                        (ar.weight as f64) - 100.0
                    } else {
                        (ar.weight as f64) - 1000.0
                    };

                    candidates.push(Candidate {
                        simplified: Arc::from(ar.word),
                        traditional: if ar.trad.is_empty() {
                            Arc::from(ar.word)
                        } else {
                            Arc::from(ar.trad)
                        },
                        text: Arc::from(ar.word),
                        hint: build_hint(&ar),
                        source: Arc::from("Table (Abbr)"),
                        weight: adjusted_weight,
                        match_level: 2,
                    });
                }
                if candidates.len() >= internal_limit {
                    break;
                }
            }
        } else {
            let results = self.trie.search_bfs(&query, internal_limit);
            for tr in results {
                if seen.insert(tr.word) {
                    candidates.push(Candidate {
                        simplified: Arc::from(tr.word),
                        traditional: if tr.trad.is_empty() {
                            Arc::from(tr.word)
                        } else {
                            Arc::from(tr.trad)
                        },
                        text: Arc::from(tr.word),
                        hint: build_hint(&tr),
                        source: Arc::from("Table"),
                        weight: tr.weight as f64,
                        match_level: 1,
                    });
                }
                if candidates.len() >= internal_limit {
                    break;
                }
            }
        }
        candidates
    }
}

/// 用户词库翻译器 (仅处理用户自造词)
pub struct UserDictTranslator {
    pub user_dict: Arc<ArcSwap<UserDictData>>,
    pub profile: String,
}
impl Translator for UserDictTranslator {
    fn translate(
        &self,
        _input: &str,
        segments: &[String],
        _config: &Config,
        _limit: usize,
    ) -> Vec<Candidate> {
        let query = segments.join("");
        let mut results = Vec::new();
        let dict = self.user_dict.load();
        if let Some(profile_dict) = dict.get(&self.profile) {
            if let Some(words) = profile_dict.get(&query) {
                for (word, weight) in words {
                    results.push(Candidate {
                        text: Arc::from(word.as_str()),
                        simplified: Arc::from(word.as_str()),
                        traditional: Arc::from(word.as_str()),
                        hint: Arc::from("User"),
                        source: Arc::from("User"),
                        weight: *weight as f64,
                        match_level: 3,
                    });
                }
            }
        }
        results
    }
}

/// 简单排序过滤器
pub struct SortFilter;
impl Filter for SortFilter {
    fn filter(
        &self,
        _input: &str,
        mut candidates: Vec<Candidate>,
        _config: &Config,
        _context: Option<&str>,
    ) -> Vec<Candidate> {
        candidates.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }
}

/// 繁简转换过滤器
pub struct TraditionalFilter;
impl Filter for TraditionalFilter {
    fn filter(
        &self,
        _input: &str,
        mut candidates: Vec<Candidate>,
        config: &Config,
        _context: Option<&str>,
    ) -> Vec<Candidate> {
        if config.input.enable_traditional {
            for c in &mut candidates {
                c.text = c.traditional.clone();
            }
        } else {
            for c in &mut candidates {
                c.text = c.simplified.clone();
            }
        }
        candidates
    }
}

/// 动态自适应过滤器 (调频与上下文联想)
pub struct AdaptiveFilter {
    pub usage_history: Arc<ArcSwap<UserDictData>>,
    pub ngram_history: Arc<ArcSwap<UserDictData>>,
    pub profile: String,
}
impl Filter for AdaptiveFilter {
    fn filter(
        &self,
        input: &str,
        mut candidates: Vec<Candidate>,
        _config: &Config,
        context: Option<&str>,
    ) -> Vec<Candidate> {
        let usage_guard = self.usage_history.load();
        let ngram_guard = self.ngram_history.load();

        // 构建 HashMap 用于 O(1) 查找，而不是 O(n) 线性搜索
        if let Some(profile_usage) = usage_guard.get(&self.profile) {
            if let Some(entries) = profile_usage.get(input) {
                let usage_map: std::collections::HashMap<&str, u32> =
                    entries.iter().map(|(w, c)| (w.as_str(), *c)).collect();
                for c in &mut candidates {
                    if let Some(&count) = usage_map.get(c.simplified.as_ref()) {
                        c.weight += (count as f64) * 1000000.0;
                    }
                }
            }
        }

        // 上下文联想 (N-Gram) 加权
        if let Some(ctx) = context {
            if let Some(profile_ngram) = ngram_guard.get(&self.profile) {
                if let Some(entries) = profile_ngram.get(ctx) {
                    let ngram_map: std::collections::HashMap<&str, u32> =
                        entries.iter().map(|(w, c)| (w.as_str(), *c)).collect();
                    for c in &mut candidates {
                        if let Some(&count) = ngram_map.get(c.simplified.as_ref()) {
                            c.weight += (count as f64) * 5000000.0;
                        }
                    }
                }
            }
        }

        // 再次根据新权重排序
        candidates.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        candidates
    }
}

/// 核心管道定义
pub struct Pipeline {
    pub segmentor: Box<dyn Segmentor>,
    pub translators: Vec<Box<dyn Translator>>,
    pub filters: Vec<Box<dyn Filter>>,
}

impl Pipeline {
    pub fn new(segmentor: Box<dyn Segmentor>) -> Self {
        Self {
            segmentor,
            translators: Vec::new(),
            filters: Vec::new(),
        }
    }

    pub fn add_translator(&mut self, t: Box<dyn Translator>) {
        self.translators.push(t);
    }

    pub fn add_filter(&mut self, f: Box<dyn Filter>) {
        self.filters.push(f);
    }

    pub fn run(
        &self,
        input: &str,
        syllables: &HashSet<String>,
        config: &Config,
        limit: usize,
        context: Option<&str>,
    ) -> Vec<Candidate> {
        let segments = self.segmentor.segment(input, syllables);
        let mut candidates = Vec::new();
        for t in &self.translators {
            candidates.extend(t.translate(input, &segments, config, limit));
        }
        for f in &self.filters {
            candidates = f.filter(input, candidates, config, context);
        }
        candidates
    }
}

/// 搜索引擎：协调所有的 Pipeline
#[derive(Clone)]
pub struct SearchEngine {
    pub trie_paths: HashMap<String, (PathBuf, PathBuf)>,
    syllables: Arc<HashSet<String>>,
    learned_words: Arc<ArcSwap<UserDictData>>,
    usage_history: Arc<ArcSwap<UserDictData>>,
    ngram_history: Arc<ArcSwap<UserDictData>>,
    pub schemes: Arc<HashMap<String, Box<dyn crate::engine::scheme::InputScheme>>>,
    pipelines: Arc<RwLock<HashMap<String, Arc<Pipeline>>>>,
}

pub struct SearchQuery<'a> {
    pub buffer: &'a str,
    pub profile: &'a str,
    pub syllables: &'a HashSet<String>,
    pub config: &'a Config,
    pub limit: usize,
    pub filter_mode: crate::engine::processor::FilterMode,
    pub aux_filter: &'a str,
    pub context: Option<&'a str>,
}

impl SearchEngine {
    pub fn new(
        trie_paths: HashMap<String, (PathBuf, PathBuf)>,
        syllables: Arc<HashSet<String>>,
        learned_words: Arc<ArcSwap<UserDictData>>,
        usage_history: Arc<ArcSwap<UserDictData>>,
        ngram_history: Arc<ArcSwap<UserDictData>>,
        schemes: Arc<HashMap<String, Box<dyn crate::engine::scheme::InputScheme>>>,
    ) -> Self {
        Self {
            trie_paths,
            syllables,
            learned_words,
            usage_history,
            ngram_history,
            schemes,
            pipelines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn search(&self, query: SearchQuery) -> (Vec<Candidate>, Vec<String>) {
        self.do_search(query)
    }

    fn do_search(&self, query: SearchQuery) -> (Vec<Candidate>, Vec<String>) {
        let span =
            tracing::info_span!("engine_search", profile = %query.profile, buffer = %query.buffer);
        let _enter = span.enter();

        if let Some(pipeline) = self.get_or_create_pipeline(query.profile) {
            let results = pipeline.run(
                query.buffer,
                query.syllables,
                query.config,
                query.limit,
                query.context,
            );
            let segments = pipeline.segmentor.segment(query.buffer, query.syllables);

            let mut final_results = results;
            if query.filter_mode == crate::engine::processor::FilterMode::Global
                && !query.aux_filter.is_empty()
            {
                final_results.retain(|c| self.matches_filter(c, query.aux_filter));
            }

            return (final_results, segments);
        }

        if let Some(scheme) = self.schemes.get(query.profile) {
            let context = crate::engine::scheme::SchemeContext {
                config: query.config,
                tries: &HashMap::new(),
                syllables: query.syllables,
                _user_dict: &Arc::new(arc_swap::ArcSwap::from_pointee(HashMap::new())),
                active_profiles: &[query.profile.to_string()],
                candidate_count: 0,
                _filter_mode: query.filter_mode.clone(),
                _aux_filter: query.aux_filter,
            };

            let pre_processed = scheme.pre_process(query.buffer, &context);
            let mut scheme_candidates = scheme.lookup(&pre_processed, &context);
            scheme.post_process(&pre_processed, &mut scheme_candidates, &context);

            let mut results = Vec::new();
            for sc in scheme_candidates {
                results.push(Candidate {
                    text: if query.config.input.enable_traditional {
                        Arc::from(sc.traditional.as_str())
                    } else {
                        Arc::from(sc.simplified.as_str())
                    },
                    simplified: Arc::from(sc.simplified.as_str()),
                    traditional: Arc::from(sc.traditional.as_str()),
                    hint: Arc::from(sc.tone.as_str()),
                    source: Arc::from("Engine"),
                    weight: sc.weight as f64,
                    match_level: sc.match_level,
                });
            }
            return (results, vec![]);
        }

        (vec![], vec![])
    }

    pub fn has_exact_match(&self, profile: &str, pinyin: &str, word: &str) -> bool {
        if let Some(paths) = self.trie_paths.get(profile) {
            if let Ok(trie) = Trie::load(&paths.0, &paths.1, true) {
                if let Some(exacts) = trie.get_all_exact(pinyin) {
                    return exacts.iter().any(|tr| tr.word == word);
                }
            }
        }
        false
    }

    fn get_or_create_pipeline(&self, profile: &str) -> Option<Arc<Pipeline>> {
        // 1. 尝试读取现有
        {
            let p_map = self.pipelines.read().ok()?;
            if let Some(p) = p_map.get(profile) {
                return Some(p.clone());
            }
        }

        // 2. 如果不存在，尝试创建
        let paths = self.trie_paths.get(profile)?;
        tracing::info!(%profile, "Lazy loading dictionary...");
        let trie = Trie::load(&paths.0, &paths.1, true).ok()?;

        let mut pipeline = Pipeline::new(Box::new(DefaultSegmentor));
        pipeline.add_translator(Box::new(UserDictTranslator {
            user_dict: self.learned_words.clone(),
            profile: profile.to_string(),
        }));
        pipeline.add_translator(Box::new(TableTranslator {
            trie: Arc::new(trie),
            syllables: self.syllables.clone(),
            // 简拼只对拼音方案有意义；笔画/英文/日文使用前缀搜索。
            enable_abbreviation: profile == "chinese",
        }));
        pipeline.add_filter(Box::new(SortFilter));
        pipeline.add_filter(Box::new(AdaptiveFilter {
            usage_history: self.usage_history.clone(),
            ngram_history: self.ngram_history.clone(),
            profile: profile.to_string(),
        }));
        pipeline.add_filter(Box::new(TraditionalFilter));

        let arc_p = Arc::new(pipeline);
        let mut p_map = self.pipelines.write().ok()?;
        p_map.insert(profile.to_string(), arc_p.clone());
        Some(arc_p)
    }

    pub fn has_longer_match(&self, profile: &str, buffer: &str) -> bool {
        if let Some(paths) = self.trie_paths.get(profile) {
            if let Ok(trie) = Trie::load(&paths.0, &paths.1, true) {
                return trie.has_longer_match(buffer);
            }
        }
        false
    }

    pub fn clear_cache(&self) {
        // No-op: 搜索缓存已移除，搜索结果直接由 Trie 和 Pipeline 计算
    }

    /// 预加载并初始化指定方案的 Pipeline
    pub fn prewarm_profile(&self, profile: &str) {
        let span = tracing::info_span!("prewarm_profile", %profile);
        let _enter = span.enter();

        // 直接调用 get_or_create_pipeline，这将触发完整的加载和缓存流程
        if let Some(_pipeline) = self.get_or_create_pipeline(profile) {
            tracing::info!(%profile, "Pipeline eagerly initialized and cached.");
            // 顺便触发一次内部 trie 的预热（如果是 Mmap 模式）
            // 虽然目前默认是全内存加载，但保留此逻辑以增强兼容性
            if let Some(paths) = self.trie_paths.get(profile) {
                if let Ok(trie) = Trie::load(&paths.0, &paths.1, true) {
                    trie.prewarm(1000);
                }
            }
        }
    }

    pub fn matches_filter(&self, candidate: &Candidate, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let filter_lower = filter.to_lowercase();
        let hint_lower = candidate.hint.to_lowercase();
        let hint_clean = crate::engine::processor::strip_tones(&hint_lower);
        let parts: Vec<&str> = hint_clean.split([' ', '/', '(', ')', ',']).collect();
        parts.iter().any(|p| p.starts_with(&filter_lower)) || hint_clean.starts_with(&filter_lower)
    }
}
