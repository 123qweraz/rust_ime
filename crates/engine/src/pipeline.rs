use crate::config_manager::UserDictData;
use crate::processor::Action;
use crate::trie::TrieResult;
use crate::EngineContext;
use crate::Trie;
use crate::Config;
use arc_swap::ArcSwap;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

const USAGE_HISTORY_WEIGHT_MULTIPLIER: f64 = 1_000_000.0;
const NGRAM_HISTORY_WEIGHT_MULTIPLIER: f64 = 5_000_000.0;
const PREWARM_ENTRIES: usize = 1000;
const MAX_LOOKUP_LIMIT: usize = 500;
const CACHE_TTL_MS: u64 = 50;

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

pub trait Translator: Send + Sync + 'static {
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
        // 快速路径：如果输入已经是小写字母/数字，直接使用（避免 to_lowercase 分配）
        let needs_lowercase = !input
            .bytes()
            .all(|b| (b >= b'a' && b <= b'z') || (b >= b'0' && b <= b'9'));

        if needs_lowercase {
            let input_lower = input.to_lowercase();
            return Self::segment_lowercase(&input_lower, syllables);
        }

        Self::segment_lowercase(input, syllables)
    }
}

impl DefaultSegmentor {
    #[inline]
    fn segment_lowercase(input: &str, syllables: &HashSet<String>) -> Vec<String> {
        let mut segments = Vec::new();
        let mut pos = 0;

        while pos < input.len() {
            let max_len = 12.min(input.len() - pos);
            let mut matched = false;

            for len in (1..=max_len).rev() {
                let end = pos + len;
                if input.is_char_boundary(end) {
                    let part = &input[pos..end];
                    if syllables.contains(part) {
                        segments.push(part.to_string());
                        pos = end;
                        matched = true;
                        break;
                    }
                }
            }

            if !matched {
                let ch = input[pos..].chars().next().unwrap();
                segments.push(ch.to_string());
                pos += ch.len_utf8();
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
    last_query: std::sync::RwLock<(String, std::time::Instant)>,
    cached_candidates: std::sync::RwLock<Vec<Candidate>>,
}

impl TableTranslator {
    pub fn new(
        trie: Arc<Trie>,
        syllables: Arc<HashSet<String>>,
        enable_abbreviation: bool,
    ) -> Self {
        Self {
            trie,
            syllables,
            enable_abbreviation,
            last_query: std::sync::RwLock::new((String::new(), std::time::Instant::now())),
            cached_candidates: std::sync::RwLock::new(Vec::new()),
        }
    }
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

        // 检查缓存是否可以复用（增量搜索优化）
        {
            let cached = self.cached_candidates.read().unwrap();
            let (last_q, last_time) = &*self.last_query.read().unwrap();

            if last_q.starts_with(&query) && last_time.elapsed().as_millis() < CACHE_TTL_MS as u128
            {
                // 新的查询是之前查询的前缀，复用缓存
                let filtered: Vec<Candidate> = cached
                    .iter()
                    .filter(|c| {
                        let word = c.simplified.as_ref();
                        word.starts_with(&query) || word.starts_with(last_q)
                    })
                    .take(limit)
                    .cloned()
                    .collect();

                if !filtered.is_empty() {
                    return filtered;
                }
            }
        }

        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        let internal_limit = limit.max(MAX_LOOKUP_LIMIT);

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

        // 更新缓存
        {
            let mut last_q = self.last_query.write().unwrap();
            *last_q = (query, std::time::Instant::now());
            let mut cached = self.cached_candidates.write().unwrap();
            *cached = candidates.clone();
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
    last_input: std::sync::RwLock<Option<(String, std::time::Instant)>>,
    cached_usage_map: std::sync::RwLock<Option<std::collections::HashMap<String, u32>>>,
    cached_ngram_map: std::sync::RwLock<Option<std::collections::HashMap<String, u32>>>,
}

impl AdaptiveFilter {
    pub fn new(
        usage_history: Arc<ArcSwap<UserDictData>>,
        ngram_history: Arc<ArcSwap<UserDictData>>,
        profile: String,
    ) -> Self {
        Self {
            usage_history,
            ngram_history,
            profile,
            last_input: std::sync::RwLock::new(None),
            cached_usage_map: std::sync::RwLock::new(None),
            cached_ngram_map: std::sync::RwLock::new(None),
        }
    }
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

        // 使用缓存的 HashMap（避免重复构建）
        if let Some(profile_usage) = usage_guard.get(&self.profile) {
            if let Some(entries) = profile_usage.get(input) {
                // 检查缓存是否有效
                let use_cached = {
                    if let Ok(guard) = self.last_input.read() {
                        if let Some((ref last_input, ref time)) = *guard {
                            *last_input == input && time.elapsed().as_millis() < 100
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if use_cached {
                    if let Ok(guard) = self.cached_usage_map.read() {
                        if let Some(ref usage_map) = *guard {
                            for c in &mut candidates {
                                if let Some(&count) = usage_map.get(c.simplified.as_ref()) {
                                    c.weight += (count as f64) * USAGE_HISTORY_WEIGHT_MULTIPLIER;
                                }
                            }
                        }
                    }
                } else {
                    // 构建并缓存 HashMap
                    let usage_map: std::collections::HashMap<String, u32> =
                        entries.iter().map(|(w, c)| (w.clone(), *c)).collect();

                    for c in &mut candidates {
                        if let Some(&count) = usage_map.get(c.simplified.as_ref()) {
                            c.weight += (count as f64) * USAGE_HISTORY_WEIGHT_MULTIPLIER;
                        }
                    }

                    // 更新缓存
                    if let Ok(mut guard) = self.cached_usage_map.write() {
                        *guard = Some(usage_map);
                    }
                    if let Ok(mut guard) = self.last_input.write() {
                        *guard = Some((input.to_string(), std::time::Instant::now()));
                    }
                }
            }
        }

        // 上下文联想 (N-Gram) 加权
        if let Some(ctx) = context {
            if let Some(profile_ngram) = ngram_guard.get(&self.profile) {
                if let Some(entries) = profile_ngram.get(ctx) {
                    let ngram_map: std::collections::HashMap<String, u32> =
                        entries.iter().map(|(w, c)| (w.clone(), *c)).collect();
                    for c in &mut candidates {
                        if let Some(&count) = ngram_map.get(c.simplified.as_ref()) {
                            c.weight += (count as f64) * NGRAM_HISTORY_WEIGHT_MULTIPLIER;
                        }
                    }

                    // 缓存 ngram map
                    if let Ok(mut guard) = self.cached_ngram_map.write() {
                        *guard = Some(ngram_map);
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
    segment_cache: std::sync::RwLock<std::collections::HashMap<String, Vec<String>>>,
}

impl Pipeline {
    pub fn new(segmentor: Box<dyn Segmentor>) -> Self {
        Self {
            segmentor,
            translators: Vec::new(),
            filters: Vec::new(),
            segment_cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn add_translator(&mut self, t: Box<dyn Translator>) {
        self.translators.push(t);
    }

    pub fn add_filter(&mut self, f: Box<dyn Filter>) {
        self.filters.push(f);
    }

    #[inline]
    pub fn run(
        &self,
        input: &str,
        syllables: &HashSet<String>,
        config: &Config,
        limit: usize,
        context: Option<&str>,
    ) -> Vec<Candidate> {
        // 使用缓存的分段结果
        let segments = {
            if let Ok(guard) = self.segment_cache.read() {
                if let Some(cached) = guard.get(input) {
                    cached.clone()
                } else {
                    drop(guard);
                    let segments = self.segmentor.segment(input, syllables);
                    if let Ok(mut guard) = self.segment_cache.write() {
                        if guard.len() < 100 {
                            guard.insert(input.to_string(), segments.clone());
                        }
                    }
                    segments
                }
            } else {
                self.segmentor.segment(input, syllables)
            }
        };

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
    pub schemes: Arc<HashMap<String, Box<dyn crate::scheme::InputScheme>>>,
    pipelines: Arc<RwLock<HashMap<String, Arc<Pipeline>>>>,
    pipeline_access_order: Arc<RwLock<Vec<String>>>,
}

const MAX_CACHED_PIPELINES: usize = 10;

pub struct SearchQuery<'a> {
    pub buffer: &'a str,
    pub profile: &'a str,
    pub syllables: &'a HashSet<String>,
    pub config: &'a Config,
    pub limit: usize,
    pub filter_mode: crate::processor::FilterMode,
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
        schemes: Arc<HashMap<String, Box<dyn crate::scheme::InputScheme>>>,
    ) -> Self {
        Self {
            trie_paths,
            syllables,
            learned_words,
            usage_history,
            ngram_history,
            schemes,
            pipelines: Arc::new(RwLock::new(HashMap::new())),
            pipeline_access_order: Arc::new(RwLock::new(Vec::new())),
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
            if query.filter_mode == crate::processor::FilterMode::Global
                && !query.aux_filter.is_empty()
            {
                final_results.retain(|c| self.matches_filter(c, query.aux_filter));
            }

            return (final_results, segments);
        }

        if let Some(scheme) = self.schemes.get(query.profile) {
            let context = crate::scheme::SchemeContext {
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

    #[inline]
    pub fn has_exact_match(&self, profile: &str, pinyin: &str, word: &str) -> bool {
        if let Some(pipeline) = self.get_or_create_pipeline(profile) {
            if let Some(trie) = self.get_trie_from_pipeline(pipeline.as_ref()) {
                if let Some(exacts) = trie.get_all_exact(pinyin) {
                    return exacts.iter().any(|tr| tr.word == word);
                }
            }
        }
        false
    }

    fn get_trie_from_pipeline(&self, pipeline: &Pipeline) -> Option<&Trie> {
        use std::any::{Any, TypeId};
        for t in &pipeline.translators {
            if TypeId::of::<TableTranslator>() == t.as_ref().type_id() {
                let ptr = t.as_ref() as *const dyn Translator;
                let table_ptr = ptr as *const TableTranslator;
                unsafe {
                    return Some(&(*table_ptr).trie);
                }
            }
        }
        None
    }

    fn get_or_create_pipeline(&self, profile: &str) -> Option<Arc<Pipeline>> {
        // 1. 尝试读取现有
        {
            let p_map = self.pipelines.read().ok()?;
            if let Some(p) = p_map.get(profile) {
                // 更新访问顺序（LRU）
                if let Ok(mut access_order) = self.pipeline_access_order.write() {
                    if let Some(pos) = access_order.iter().position(|p| p == profile) {
                        access_order.remove(pos);
                    }
                    access_order.push(profile.to_string());
                }
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
        pipeline.add_translator(Box::new(TableTranslator::new(
            Arc::new(trie),
            self.syllables.clone(),
            profile == "chinese",
        )));
        pipeline.add_filter(Box::new(SortFilter));
        pipeline.add_filter(Box::new(AdaptiveFilter::new(
            self.usage_history.clone(),
            self.ngram_history.clone(),
            profile.to_string(),
        )));
        pipeline.add_filter(Box::new(TraditionalFilter));

        let arc_p = Arc::new(pipeline);

        // LRU eviction: 如果缓存超过限制，移除最久未使用的
        {
            let maybe_oldest = {
                let access_order = self.pipeline_access_order.read().ok()?;
                if access_order.len() >= MAX_CACHED_PIPELINES {
                    access_order.first().cloned()
                } else {
                    None
                }
            };

            if let Some(oldest) = maybe_oldest {
                if let Ok(mut p_map) = self.pipelines.write() {
                    p_map.remove(&oldest);
                }
                if let Ok(mut access_order) = self.pipeline_access_order.write() {
                    access_order.remove(0);
                }
                tracing::debug!(profile = %oldest, "Evicted pipeline from cache");
            }
        }

        // 插入新 pipeline
        let mut p_map = self.pipelines.write().ok()?;
        p_map.insert(profile.to_string(), arc_p.clone());

        if let Ok(mut access_order) = self.pipeline_access_order.write() {
            if !access_order.contains(&profile.to_string()) {
                access_order.push(profile.to_string());
            }
        }

        Some(arc_p)
    }

    #[inline]
    pub fn has_longer_match(&self, profile: &str, buffer: &str) -> bool {
        if let Some(pipeline) = self.get_or_create_pipeline(profile) {
            if let Some(trie) = self.get_trie_from_pipeline(pipeline.as_ref()) {
                return trie.has_longer_match(buffer);
            }
        }
        false
    }

    pub fn clear_cache(&self) {
        if let Ok(mut p_map) = self.pipelines.write() {
            p_map.clear();
        }
        if let Ok(mut access_order) = self.pipeline_access_order.write() {
            access_order.clear();
        }
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
                    trie.prewarm(PREWARM_ENTRIES);
                }
            }
        }
    }

    #[inline]
    pub fn matches_filter(&self, candidate: &Candidate, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        let filter_lower = filter.to_lowercase();
        let hint_lower = candidate.hint.to_lowercase();
        let hint_clean = crate::processor::strip_tones(&hint_lower);
        let parts: Vec<&str> = hint_clean.split([' ', '/', '(', ')', ',']).collect();
        parts.iter().any(|p| p.starts_with(&filter_lower)) || hint_clean.starts_with(&filter_lower)
    }
}

pub fn lookup(ctx: &mut EngineContext) -> Option<Action> {
    use crate::processor::FilterMode;
    use std::sync::Arc;

    if ctx.session.buffer.is_empty() {
        ctx.reset();
        return None;
    }

    if ctx.session.filter_mode == FilterMode::Page && !ctx.session.page_snapshot.is_empty() {
        let mut filtered = Vec::new();
        for c in &ctx.session.page_snapshot {
            if ctx.engine.matches_filter(c, &ctx.session.aux_filter) {
                filtered.push(c.clone());
            }
        }
        if !filtered.is_empty() {
            ctx.session.candidates = filtered;
            if ctx.session.candidates.len() == 1 {
                let word = ctx.session.candidates[0].text.clone();
                return Some(crate::processor::commands::commit_candidate(
                    ctx, word, 0,
                ));
            }
        } else {
            ctx.session.candidates.clear();
        }
        ctx.session.update_state();
        return None;
    }

    let current_profile = ctx
        .session_state
        .active_profiles
        .first()
        .cloned()
        .unwrap_or_default();
    let last_word = ctx
        .session_state
        .commit_history
        .last()
        .map(|(_, word)| word.as_str());

    let query = SearchQuery {
        buffer: &ctx.session.buffer,
        profile: &current_profile,
        syllables: &ctx.syllables,
        config: &ctx.config.master_config,
        limit: 20,
        filter_mode: ctx.session.filter_mode.clone(),
        aux_filter: &ctx.session.aux_filter,
        context: last_word,
    };
    let (results, segments) = ctx.engine.search(query);
    ctx.session.candidates = results;
    ctx.session.best_segmentation = segments;
    ctx.session.has_dict_match = !ctx.session.candidates.is_empty();
    ctx.session.last_lookup_pinyin = ctx.session.buffer.clone();

    if ctx.session.candidates.len() == 1 && ctx.session.filter_mode == FilterMode::Global {
        let word = ctx.session.candidates[0].text.clone();
        return Some(crate::processor::commands::commit_candidate(
            ctx, word, 0,
        ));
    }

    if ctx.session.candidates.is_empty() {
        let buf_arc: Arc<str> = Arc::from(ctx.session.buffer.as_str());
        ctx.session.candidates.push(Candidate {
            text: buf_arc.clone(),
            simplified: buf_arc.clone(),
            traditional: buf_arc.clone(),
            hint: Arc::from(""),
            source: Arc::from("Raw"),
            weight: 0.0,
            match_level: 0,
        });
    }
    ctx.session.update_state();
    crate::compositor::Compositor::check_auto_commit(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;

    #[test]
    fn test_default_segmentor_basic() {
        let segmentor = DefaultSegmentor;
        let syllables: HashSet<String> = ["ni", "hao", "zhong", "guo"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let result = segmentor.segment("nihao", &syllables);
        assert_eq!(result, vec!["ni", "hao"]);
    }

    #[test]
    fn test_default_segmentor_longer_match() {
        let segmentor = DefaultSegmentor;
        let syllables: HashSet<String> = ["zhong", "guo", "zhongguo"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let result = segmentor.segment("zhongguo", &syllables);
        assert_eq!(result, vec!["zhongguo"]);
    }

    #[test]
    fn test_default_segmentor_partial_match() {
        let segmentor = DefaultSegmentor;
        let syllables: HashSet<String> = ["zhong", "guo"].iter().map(|s| s.to_string()).collect();

        let result = segmentor.segment("zhongguo", &syllables);
        assert_eq!(result, vec!["zhong", "guo"]);
    }

    #[test]
    fn test_default_segmentor_unknown_chars() {
        let segmentor = DefaultSegmentor;
        let syllables: HashSet<String> = ["ni"].iter().map(|s| s.to_string()).collect();

        let result = segmentor.segment("nixyz", &syllables);
        assert_eq!(result, vec!["ni", "x", "y", "z"]);
    }

    #[test]
    fn test_default_segmentor_empty_input() {
        let segmentor = DefaultSegmentor;
        let syllables: HashSet<String> = ["ni", "hao"].iter().map(|s| s.to_string()).collect();

        let result = segmentor.segment("", &syllables);
        assert!(result.is_empty());
    }

    #[test]
    fn test_candidate_clone() {
        let candidate = Candidate {
            text: Arc::from("test"),
            simplified: Arc::from("test"),
            traditional: Arc::from("test"),
            hint: Arc::from("hint"),
            source: Arc::from("test"),
            weight: 1.0,
            match_level: 3,
        };

        let cloned = candidate.clone();
        assert_eq!(candidate.text, cloned.text);
        assert_eq!(candidate.weight, cloned.weight);
    }

    #[test]
    fn test_sort_filter() {
        let filter = SortFilter;
        let candidates = vec![
            Candidate {
                text: Arc::from("low"),
                simplified: Arc::from("low"),
                traditional: Arc::from("low"),
                hint: Arc::from(""),
                source: Arc::from(""),
                weight: 1.0,
                match_level: 1,
            },
            Candidate {
                text: Arc::from("high"),
                simplified: Arc::from("high"),
                traditional: Arc::from("high"),
                hint: Arc::from(""),
                source: Arc::from(""),
                weight: 100.0,
                match_level: 1,
            },
            Candidate {
                text: Arc::from("medium"),
                simplified: Arc::from("medium"),
                traditional: Arc::from("medium"),
                hint: Arc::from(""),
                source: Arc::from(""),
                weight: 50.0,
                match_level: 1,
            },
        ];

        let config = Config::default_config();
        let result = filter.filter("test", candidates, &config, None);
        assert_eq!(result[0].text.as_ref(), "high");
        assert_eq!(result[1].text.as_ref(), "medium");
        assert_eq!(result[2].text.as_ref(), "low");
    }

    #[test]
    fn test_traditional_filter_simplified() {
        let filter = TraditionalFilter;
        let candidates = vec![Candidate {
            text: Arc::from("简化"),
            simplified: Arc::from("简化"),
            traditional: Arc::from("簡化"),
            hint: Arc::from(""),
            source: Arc::from(""),
            weight: 1.0,
            match_level: 1,
        }];

        let config = Config::default_config();
        let result = filter.filter("test", candidates, &config, None);
        assert_eq!(result[0].text.as_ref(), "简化");
    }

    #[test]
    fn test_traditional_filter_traditional() {
        let filter = TraditionalFilter;
        let mut config = Config::default_config();
        config.input.enable_traditional = true;

        let candidates = vec![Candidate {
            text: Arc::from("简化"),
            simplified: Arc::from("简化"),
            traditional: Arc::from("簡化"),
            hint: Arc::from(""),
            source: Arc::from(""),
            weight: 1.0,
            match_level: 1,
        }];

        let result = filter.filter("test", candidates, &config, None);
        assert_eq!(result[0].text.as_ref(), "簡化");
    }

    #[test]
    fn test_matches_filter_empty() {
        let engine = create_test_engine();
        let candidate = Candidate {
            text: Arc::from("测试"),
            simplified: Arc::from("测试"),
            traditional: Arc::from("测试"),
            hint: Arc::from("ceshi"),
            source: Arc::from(""),
            weight: 1.0,
            match_level: 1,
        };

        assert!(engine.matches_filter(&candidate, ""));
        assert!(engine.matches_filter(&candidate, "ces"));
        assert!(!engine.matches_filter(&candidate, "xyz"));
    }

    fn create_test_engine() -> SearchEngine {
        SearchEngine::new(
            HashMap::new(),
            Arc::new(HashSet::new()),
            Arc::new(ArcSwap::from_pointee(HashMap::new())),
            Arc::new(ArcSwap::from_pointee(HashMap::new())),
            Arc::new(ArcSwap::from_pointee(HashMap::new())),
            Arc::new(HashMap::new()),
        )
    }
}
