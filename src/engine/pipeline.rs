use crate::Config;
use std::collections::{HashSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use crate::engine::Trie;
use crate::engine::config_manager::UserDictData;
use lru::LruCache;

#[derive(Hash, PartialEq, Eq, Clone)]
struct SearchCacheKey {
    profile: String,
    buffer: String,
    limit: usize,
    filter_mode: crate::engine::processor::FilterMode,
    aux_filter: String,
}
// use crate::engine::keys::VirtualKey;

/// 候选词元数据
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub text: Arc<str>,
    pub simplified: Arc<str>,
    pub traditional: Arc<str>,
    pub hint: Arc<str>,
    pub source: Arc<str>, // 来源：如 "User", "Table", "Script"
    pub weight: f64,
}

/*
/// 1. 预处理器：按键到字符串映射的转换
pub trait Preprocessor: Send {
    fn process(&self, key: VirtualKey, shift: bool, buffer: &mut String) -> bool;
}
*/

/// 2. 切分器：字符串到音节序列的转换
pub trait Segmentor: Send + Sync {
    fn segment(&self, input: &str, syllables: &HashSet<String>) -> Vec<String>;
}

/// 3. 翻译器：音节到候选词的转换
pub trait Translator: Send + Sync {
    fn translate(&self, input: &str, segments: &[String], config: &Config, limit: usize) -> Vec<Candidate>;
}

/// 4. 过滤器：对候选词列表的后期加工
pub trait Filter: Send + Sync {
    fn filter(&self, candidates: &mut Vec<Candidate>, config: &Config, query: &str, context: Option<&str>);
}

/// 默认切分器实现 (Max Match)
pub struct DefaultSegmentor;
impl Segmentor for DefaultSegmentor {
    fn segment(&self, input: &str, syllables: &HashSet<String>) -> Vec<String> {
        let mut segments = Vec::new();
        let mut remaining = input.to_lowercase();

        while !remaining.is_empty() {
            let mut matched = false;
            // 尝试最长匹配音节 (Max Match)
            // 提高上限到 12 以涵盖长词组 Key (如 zhuomian)
            for len in (1..=12).rev() {
                if len <= remaining.len() {
                    let part = &remaining[..len];
                    if syllables.contains(part) {
                        segments.push(part.to_string());
                        remaining = remaining[len..].to_string();
                        matched = true;
                        break;
                    }
                }
            }
            if !matched {
                // 如果没有任何音节匹配，按单字母切分（简拼或未知输入）
                if let Some(first_char) = remaining.chars().next() {
                    segments.push(first_char.to_string());
                    remaining = remaining[first_char.len_utf8()..].to_string();
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
}
impl Translator for TableTranslator {
    fn translate(&self, _input: &str, segments: &[String], config: &Config, limit: usize) -> Vec<Candidate> {
        if segments.is_empty() { return vec![]; }
        let query = segments.join("");
        let mut candidates = Vec::new();
        
        // 性能优化：在内部使用更大的搜索深度（如 500），以确保高频词能进入初选名单参与排序。
        // 否则，如果 limit 很小（如 10），字典序靠后的高频词将永远无法排到前面。
        let internal_limit = limit.max(500);

        // 1. 尝试全拼精确匹配
        if let Some(exact_results) = self.trie.get_all_exact(&query) {
            for tr in exact_results {
                let mut hint = String::new();
                if config.appearance.show_english_aux && !tr.en.is_empty() { hint.push_str(tr.en); }
                if config.appearance.show_stroke_aux && !tr.stroke_aux.is_empty() {
                    if !hint.is_empty() { hint.push(' '); }
                    hint.push_str(tr.stroke_aux);
                }
                if hint.is_empty() { hint = tr.tone.to_string(); }

                candidates.push(Candidate {
                    simplified: Arc::from(tr.word),
                    traditional: if tr.trad.is_empty() { Arc::from(tr.word) } else { Arc::from(tr.trad) },
                    text: Arc::from(tr.word), 
                    hint: Arc::from(hint.as_str()), 
                    source: Arc::from("Table (Exact)"),
                    weight: tr.weight as f64 + config.input.ranking.exact_match_bonus, 
                });
            }
        }
        
        // 判断是否为简拼输入
        let is_abbreviation = segments.len() > 1 && segments.iter().any(|s| s.len() == 1);

        if is_abbreviation && config.input.enable_abbreviation_matching {
            // 2. 执行严格简拼搜索
            let abbr_results = self.trie.search_abbreviation(segments, &self.syllables, internal_limit);
            for ar in abbr_results {
                if !candidates.iter().any(|r| r.simplified.as_ref() == ar.word) {
                    let mut hint = String::new();
                    if config.appearance.show_english_aux && !ar.en.is_empty() { hint.push_str(ar.en); }
                    if config.appearance.show_stroke_aux && !ar.stroke_aux.is_empty() {
                        if !hint.is_empty() { hint.push(' '); }
                        hint.push_str(ar.stroke_aux);
                    }
                    if hint.is_empty() { hint = ar.tone.to_string(); }
                    
                    let adjusted_weight = if ar.weight > 8000 {
                        (ar.weight as f64) - 10.0 
                    } else if ar.weight > 5000 {
                        (ar.weight as f64) - 100.0
                    } else {
                        (ar.weight as f64) - 1000.0
                    };

                    candidates.push(Candidate {
                        simplified: Arc::from(ar.word),
                        traditional: if ar.trad.is_empty() { Arc::from(ar.word) } else { Arc::from(ar.trad) },
                        text: Arc::from(ar.word), 
                        hint: Arc::from(hint.as_str()), 
                        source: Arc::from("Table (Abbr)"),
                        weight: adjusted_weight, 
                    });
                }
                if candidates.len() >= internal_limit { break; } 
            }
        } else {
            // 3. 全拼前缀补全
            let results = self.trie.search_bfs(&query, internal_limit);
            for tr in results {
                if candidates.iter().any(|c| c.simplified.as_ref() == tr.word) { continue; }
                let mut hint = String::new();
                if config.appearance.show_english_aux && !tr.en.is_empty() { hint.push_str(tr.en); }
                if config.appearance.show_stroke_aux && !tr.stroke_aux.is_empty() {
                    if !hint.is_empty() { hint.push(' '); }
                    hint.push_str(tr.stroke_aux);
                }
                if hint.is_empty() { hint = tr.tone.to_string(); }

                candidates.push(Candidate {
                    simplified: Arc::from(tr.word),
                    traditional: if tr.trad.is_empty() { Arc::from(tr.word) } else { Arc::from(tr.trad) },
                    text: Arc::from(tr.word), 
                    hint: Arc::from(hint.as_str()), 
                    source: Arc::from("Table"),
                    weight: tr.weight as f64,
                });
                if candidates.len() >= internal_limit { break; }
            }
        }
        candidates
    }
}

/// 用户词库翻译器 (仅处理用户自造词)
pub struct UserDictTranslator {
    pub user_dict: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub profile: String,
}
impl Translator for UserDictTranslator {
    fn translate(&self, _input: &str, segments: &[String], _config: &Config, _limit: usize) -> Vec<Candidate> {
        let query = segments.join("");
        let mut results = Vec::new();
        let dict = self.user_dict.load();
        if let Some(profile_dict) = dict.get(&self.profile) {
            if let Some(words) = profile_dict.get(&query) {
                for (text, freq) in words {
                    results.push(Candidate {
                        simplified: Arc::from(text.as_str()),
                        traditional: Arc::from(text.as_str()),
                        text: Arc::from(text.as_str()),
                        hint: Arc::from("★"),
                        source: Arc::from("User"),
                        weight: (*freq as f64) + 10000.0, // 基础分
                    });
                }
            }
        }
        results
    }
}

/// 调频过滤器：根据用户历史频率对已有候选词进行动态评分加成
pub struct AdaptiveFilter {
    pub usage_history: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub ngram_history: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub profile: String,
}
impl Filter for AdaptiveFilter {
    fn filter(&self, candidates: &mut Vec<Candidate>, config: &Config, query: &str, context: Option<&str>) {
        if !config.input.enable_auto_reorder { return; }

        // 1. 上下文联想提权 (N-Gram)
        if let Some(ctx) = context {
            let dict = self.ngram_history.load();
            if let Some(profile_dict) = dict.get(&self.profile) {
                if let Some(history_entries) = profile_dict.get(ctx) {
                    // 对于匹配上下文的词，给予物理置顶
                    for (word, _) in history_entries.iter().rev() {
                        if let Some(pos) = candidates.iter().position(|c| c.simplified.as_ref() == word || c.traditional.as_ref() == word) {
                            let mut cand = candidates.remove(pos);
                            if !cand.source.contains("(Context)") {
                                cand.source = format!("{} (Context)", cand.source).into();
                            }
                            candidates.insert(0, cand);
                        }
                    }
                }
            }
        }

        // 2. 基础调频逻辑 (MRU)
        let dict = self.usage_history.load();
        if let Some(profile_dict) = dict.get(&self.profile) {
            if let Some(history_entries) = profile_dict.get(query) {
                // 物理挪动：从后往前遍历历史记录，依次插入到第 0 位
                for (word, _) in history_entries.iter().rev() {
                    if let Some(pos) = candidates.iter().position(|c| c.simplified.as_ref() == word || c.traditional.as_ref() == word) {
                        let mut cand = candidates.remove(pos);
                        // 如果已经有了 Context 标记，不再覆盖，但仍保持置顶状态
                        if !cand.source.contains("(Hist)") && !cand.source.contains("(Context)") {
                            cand.source = format!("{} (Hist)", cand.source).into();
                        }
                        candidates.insert(0, cand);
                    }
                }
            }
        }
    }
}

/// 排序与去重过滤器
pub struct SortFilter;
impl Filter for SortFilter {
    fn filter(&self, candidates: &mut Vec<Candidate>, config: &Config, _query: &str, _context: Option<&str>) {
        let ranking = &config.input.ranking;
        // 纯粹基于质量的排序：词库分 + 长度惩罚
        candidates.sort_by(|a, b| {
            let mut score_a = a.weight;
            let mut score_b = b.weight;

            // 针对拼音输入优化：单字加成
            if a.text.chars().count() == 1 { score_a += ranking.single_char_bonus; }
            if b.text.chars().count() == 1 { score_b += ranking.single_char_bonus; }

            // 惩罚过长的词
            score_a -= (a.text.chars().count() as f64) * ranking.length_penalty;
            score_b -= (b.text.chars().count() as f64) * ranking.length_penalty;

            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // 去重逻辑
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.simplified.clone()));
    }
}

/// 繁简转换过滤器
pub struct TraditionalFilter;
impl Filter for TraditionalFilter {
    fn filter(&self, candidates: &mut Vec<Candidate>, config: &Config, _query: &str, _context: Option<&str>) {
        let use_trad = config.input.enable_traditional;
        for c in candidates.iter_mut() {
            c.text = if use_trad { c.traditional.clone() } else { c.simplified.clone() };
        }
    }
}

/// 核心流水线：管理并执行整个输入处理流程
pub struct Pipeline {
    // pub preprocessors: Vec<Box<dyn Preprocessor>>,
    pub segmentor: Box<dyn Segmentor>,
    pub translators: Vec<Box<dyn Translator>>,
    pub filters: Vec<Box<dyn Filter>>,
}

impl Pipeline {
    pub fn new(segmentor: Box<dyn Segmentor>) -> Self {
        Self {
            // preprocessors: vec![],
            segmentor,
            translators: vec![],
            filters: vec![],
        }
    }

    /*
    pub fn add_preprocessor(&mut self, p: Box<dyn Preprocessor>) { self.preprocessors.push(p); }
    */
    pub fn add_translator(&mut self, t: Box<dyn Translator>) { self.translators.push(t); }
    pub fn add_filter(&mut self, f: Box<dyn Filter>) { self.filters.push(f); }

    /*
    pub fn run_preprocessors(&self, key: VirtualKey, shift: bool, buffer: &mut String) -> bool {
        for p in &self.preprocessors {
            if p.process(key, shift, buffer) { return true; }
        }
        false
    }
    */

    pub fn run(&self, input: &str, syllables: &HashSet<String>, config: &Config, limit: usize, context: Option<&str>) -> Vec<Candidate> {
        let segments = self.segmentor.segment(input, syllables);
        let mut candidates = Vec::new();
        for t in &self.translators {
            candidates.extend(t.translate(input, &segments, config, limit));
        }
        for f in &self.filters {
            f.filter(&mut candidates, config, input, context);
        }
        
        // 在所有排序和过滤完成后，执行最终截断
        if candidates.len() > limit {
            candidates.truncate(limit);
        }
        
        candidates
    }
}

#[derive(Clone)]
pub struct SearchEngine {
    pub trie_paths: HashMap<String, (PathBuf, PathBuf)>,
    pub syllables: Arc<HashSet<String>>,
    pub learned_words: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub usage_history: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub ngram_history: Arc<arc_swap::ArcSwap<UserDictData>>,
    pub schemes: Arc<HashMap<String, Box<dyn crate::engine::scheme::InputScheme>>>,
    pipelines: Arc<RwLock<HashMap<String, Arc<Pipeline>>>>,
    cache: Arc<Mutex<LruCache<SearchCacheKey, (Vec<Candidate>, Vec<String>)>>>,
}

pub struct SearchQuery<'a> {
    pub buffer: &'a str,
    pub profile: &'a str,
    pub syllables: &'a HashSet<String>,
    pub config: &'a crate::Config,
    pub limit: usize,
    pub filter_mode: crate::engine::processor::FilterMode,
    pub aux_filter: &'a str,
    pub context: Option<&'a str>,
}

impl SearchEngine {
    pub fn new(
        trie_paths: HashMap<String, (PathBuf, PathBuf)>,
        syllables: Arc<HashSet<String>>,
        learned_words: Arc<arc_swap::ArcSwap<UserDictData>>,
        usage_history: Arc<arc_swap::ArcSwap<UserDictData>>,
        ngram_history: Arc<arc_swap::ArcSwap<UserDictData>>,
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
            cache: Arc::new(Mutex::new(LruCache::new(std::num::NonZeroUsize::new(1000).unwrap()))),
        }
    }



    pub fn has_exact_match(&self, profile: &str, pinyin: &str, word: &str) -> bool {
        if let Some(paths) = self.trie_paths.get(profile) {
            if let Ok(trie) = Trie::load(&paths.0, &paths.1) {
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
        let trie = Trie::load(&paths.0, &paths.1).ok()?;
        
        let mut pipeline = Pipeline::new(Box::new(DefaultSegmentor));
        pipeline.add_translator(Box::new(UserDictTranslator { 
            user_dict: self.learned_words.clone(), 
            profile: profile.to_string() 
        }));
        pipeline.add_translator(Box::new(TableTranslator { 
            trie: Arc::new(trie),
            syllables: self.syllables.clone(),
        }));
        pipeline.add_filter(Box::new(SortFilter));
        pipeline.add_filter(Box::new(AdaptiveFilter {
            usage_history: self.usage_history.clone(),
            ngram_history: self.ngram_history.clone(),
            profile: profile.to_string()
        }));
        pipeline.add_filter(Box::new(TraditionalFilter));

        let arc_p = Arc::new(pipeline);
        let mut p_map = self.pipelines.write().ok()?;
        p_map.insert(profile.to_string(), arc_p.clone());
        Some(arc_p)
    }

    pub fn has_longer_match(&self, profile: &str, buffer: &str) -> bool {
        if let Some(paths) = self.trie_paths.get(profile) {
            if let Ok(trie) = Trie::load(&paths.0, &paths.1) {
                return trie.has_longer_match(buffer);
            }
        }
        false
    }

    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// 预热指定方案的词库
    pub fn prewarm_profile(&self, profile: &str) {
        if let Some(pipeline) = self.get_or_create_pipeline(profile) {
            // 获取第一个 TableTranslator 并执行预热
            if !pipeline.translators.is_empty() {
                if let Some(paths) = self.trie_paths.get(profile) {
                    if let Ok(trie) = Trie::load(&paths.0, &paths.1) {
                        trie.prewarm(5000); // 预热前 5000 个词条
                    }
                }
            }
        }
    }

    pub fn search(
        &self,
        query: SearchQuery,
    ) -> (Vec<Candidate>, Vec<String>) {
        let key = SearchCacheKey {
            profile: query.profile.to_string(),
            buffer: query.buffer.to_string(),
            limit: query.limit,
            filter_mode: query.filter_mode.clone(),
            aux_filter: query.aux_filter.to_string(),
        };

        if let Ok(mut cache) = self.cache.lock() {
            if let Some(hit) = cache.get(&key) {
                return hit.clone();
            }
        }

        let result = self.do_search(query);
        
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(key, result.clone());
        }
        
        result
    }

    fn do_search(
        &self,
        query: SearchQuery,
    ) -> (Vec<Candidate>, Vec<String>) {
        let span = tracing::info_span!("engine_search", profile = %query.profile, buffer = %query.buffer);
        let _enter = span.enter();

        if let Some(pipeline) = self.get_or_create_pipeline(query.profile) {
            let segments = pipeline.segmentor.segment(query.buffer, query.syllables);
            let results = pipeline.run(query.buffer, query.syllables, query.config, query.limit, query.context);
            
            let mut final_results = results;
            if query.filter_mode == crate::engine::processor::FilterMode::Global && !query.aux_filter.is_empty() {
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
                    text: if query.config.input.enable_traditional { Arc::from(sc.traditional.as_str()) } else { Arc::from(sc.simplified.as_str()) },
                    simplified: Arc::from(sc.simplified.as_str()),
                    traditional: Arc::from(sc.traditional.as_str()),
                    hint: Arc::from(sc.tone.as_str()),
                    source: Arc::from("Engine"),
                    weight: sc.weight as f64,
                });
            }

            if query.filter_mode == crate::engine::processor::FilterMode::Global && !query.aux_filter.is_empty() {
                results.retain(|c| self.matches_filter(c, query.aux_filter));
            }
            return (results, vec![]); 
        }

        (vec![], vec![])
    }

    pub fn matches_filter(&self, cand: &Candidate, filter: &str) -> bool {
        if filter.is_empty() { return true; }
        let filter_lower = filter.to_lowercase();
        let hint_lower = cand.hint.to_lowercase();
        let hint_clean = crate::engine::processor::strip_tones(&hint_lower);
        let parts: Vec<&str> = hint_clean.split([' ', '/', '(', ')', ',']).collect();
        parts.iter().any(|p| p.starts_with(&filter_lower)) || hint_clean.starts_with(&filter_lower)
    }
}
