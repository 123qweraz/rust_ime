use fst::{Automaton, IntoStreamer, Map, Streamer};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

const ABBREVIATION_SCAN_LIMIT: usize = 3000;

#[derive(Clone, Copy)]
pub struct TrieResult<'a> {
    pub word: &'a str,
    pub trad: &'a str,
    pub tone: &'a str,
    pub en: &'a str,
    pub stroke_aux: &'a str,
    pub weight: u32,
}

#[derive(Clone)]
pub enum TrieData {
    Mmap(Arc<Mmap>),
    Memory(Arc<Vec<u8>>),
}

impl AsRef<[u8]> for TrieData {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Mmap(m) => m.as_ref(),
            Self::Memory(v) => v.as_ref(),
        }
    }
}

#[derive(Clone)]
pub struct Trie {
    index: Map<TrieData>,
    data: TrieData,
}

impl Trie {
    pub fn load<P: AsRef<Path>>(
        index_path: P,
        data_path: P,
        force_memory: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let load_data = |path: &Path| -> Result<TrieData, Box<dyn std::error::Error>> {
            if force_memory {
                let buffer = std::fs::read(path)?;
                Ok(TrieData::Memory(Arc::new(buffer)))
            } else {
                let file = File::open(path)?;
                let mmap = unsafe { Mmap::map(&file)? };
                Ok(TrieData::Mmap(Arc::new(mmap)))
            }
        };

        let index_data = load_data(index_path.as_ref())?;
        let data_data = load_data(data_path.as_ref())?;
        let index = Map::new(index_data)?;

        Ok(Self {
            index,
            data: data_data,
        })
    }

    pub fn get_all_exact(&self, pinyin: &str) -> Option<Vec<TrieResult<'_>>> {
        self.get_all_exact_with_level_filter(pinyin, None)
    }

    /// 带等级过滤的精确匹配
    pub fn get_all_exact_with_level_filter(
        &self,
        pinyin: &str,
        level_filter: Option<&str>,
    ) -> Option<Vec<TrieResult<'_>>> {
        let _span = tracing::debug_span!("trie_exact", %pinyin, ?level_filter).entered();
        let offset = self.index.get(pinyin)? as usize;
        let mut results = Vec::new();
        self.read_block(offset, |tr| {
            // 应用等级过滤
            if let Some(filter_level) = level_filter {
                if tr.stroke_aux == filter_level {
                    results.push(tr);
                }
            } else {
                results.push(tr);
            }
        });
        // 如果过滤后没有结果，返回 None
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    }

    /// 预热词库：读取前 limit 条记录以填充 Page Cache
    pub fn prewarm(&self, limit: usize) {
        if matches!(self.data, TrieData::Memory(_)) {
            return;
        }
        let mut stream = self.index.stream();
        let mut count = 0;
        while let Some((_, offset)) = fst::Streamer::next(&mut stream) {
            self.read_block(offset as usize, |_| {});
            count += 1;
            if count >= limit {
                break;
            }
        }
    }

    #[allow(dead_code)]
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let matcher = fst::automaton::Str::new(prefix).starts_with();
        self.index.search(matcher).into_stream().next().is_some()
    }

    pub fn has_longer_match(&self, prefix: &str) -> bool {
        let matcher = fst::automaton::Str::new(prefix).starts_with();
        let mut stream = self.index.search(matcher).into_stream();
        while let Some((key, _)) = stream.next() {
            if key.len() > prefix.len() {
                return true;
            }
        }
        false
    }

    pub fn search_bfs(&self, prefix: &str, limit: usize) -> Vec<TrieResult<'_>> {
        self.search_bfs_with_level_filter(prefix, limit, None)
    }

    /// 带等级过滤的前缀搜索
    /// level_filter: Some("level-1") 表示只返回 level-1 的结果，None 表示返回所有结果
    pub fn search_bfs_with_level_filter(
        &self,
        prefix: &str,
        limit: usize,
        level_filter: Option<&str>,
    ) -> Vec<TrieResult<'_>> {
        let _span = tracing::debug_span!("trie_bfs", %prefix, limit, ?level_filter).entered();
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 支持通配符 z：将其转换为正则搜索
        if prefix.contains('z') {
            return self.search_wildcard_with_level_filter(prefix, limit, level_filter);
        }

        let matcher = fst::automaton::Str::new(prefix).starts_with();
        let mut stream = self.index.search(matcher).into_stream();

        while let Some((_, offset)) = stream.next() {
            let mut stop = false;
            self.read_block(offset as usize, |pair| {
                if !stop && seen.insert(pair.word) {
                    // 应用等级过滤
                    if let Some(filter_level) = level_filter {
                        if pair.stroke_aux == filter_level {
                            results.push(pair);
                            if results.len() >= limit {
                                stop = true;
                            }
                        }
                    } else {
                        results.push(pair);
                        if results.len() >= limit {
                            stop = true;
                        }
                    }
                }
            });
            if stop {
                break;
            }
        }
        results
    }

    /// 通配符搜索实现：z 匹配任意单个 a-y 字母
    pub fn search_wildcard(&self, pattern: &str, limit: usize) -> Vec<TrieResult<'_>> {
        self.search_wildcard_with_level_filter(pattern, limit, None)
    }

    /// 带等级过滤的通配符搜索
    pub fn search_wildcard_with_level_filter(
        &self,
        pattern: &str,
        limit: usize,
        level_filter: Option<&str>,
    ) -> Vec<TrieResult<'_>> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 简单的 DFS 实现通配符匹配
        let mut stream = self.index.stream();
        while let Some((key_bytes, offset)) = stream.next() {
            let key = String::from_utf8_lossy(key_bytes);
            if self.wildcard_match(pattern, &key) {
                let mut stop = false;
                self.read_block(offset as usize, |pair| {
                    if !stop && seen.insert(pair.word) {
                        // 应用等级过滤
                        if let Some(filter_level) = level_filter {
                            if pair.stroke_aux == filter_level {
                                results.push(pair);
                                if results.len() >= limit {
                                    stop = true;
                                }
                            }
                        } else {
                            results.push(pair);
                            if results.len() >= limit {
                                stop = true;
                            }
                        }
                    }
                });
                if stop {
                    break;
                }
            }
        }
        results
    }

    fn wildcard_match(&self, pattern: &str, key: &str) -> bool {
        let p_chars: Vec<char> = pattern.chars().collect();
        let k_chars: Vec<char> = key.chars().collect();

        // 如果 pattern 不包含通配符且不是 key 的前缀，快速失败
        if !pattern.contains('z') {
            return key.starts_with(pattern);
        }

        // 简易正则逻辑：z 匹配任意 1 个字符
        if p_chars.len() > k_chars.len() {
            return false;
        }

        for i in 0..p_chars.len() {
            if p_chars[i] != 'z' && p_chars[i] != k_chars[i] {
                return false;
            }
        }
        true
    }

    pub fn search_abbreviation(
        &self,
        segments: &[String],
        syllables: &std::collections::HashSet<String>,
        limit: usize,
    ) -> Vec<TrieResult<'_>> {
        if segments.is_empty() {
            return Vec::new();
        }
        let mut results = Vec::with_capacity(limit);
        let mut seen = std::collections::HashSet::new();

        let first_seg = &segments[0];
        let matcher = fst::automaton::Str::new(first_seg).starts_with();
        let mut stream = self.index.search(matcher).into_stream();

        while let Some((key_bytes, offset)) = stream.next() {
            let key = String::from_utf8_lossy(key_bytes);

            // 严格匹配：
            // 1. 每一个 segment 必须匹配一个音节的开头
            // 2. 必须刚好匹配完所有 segment 且 耗尽 key 中的所有音节
            if self.matches_strict_jianpin(&key, segments, syllables) {
                let mut stop = false;
                self.read_block(offset as usize, |pair| {
                    if !stop && seen.insert(pair.word) {
                        results.push(pair);
                        if results.len() >= ABBREVIATION_SCAN_LIMIT {
                            stop = true;
                        }
                    }
                });
                if stop {
                    break;
                }
            }
            if results.len() >= ABBREVIATION_SCAN_LIMIT {
                break;
            }
        }
        results
    }

    /// 严格简拼匹配：输入 segments 数量必须等于词组音节数
    fn matches_strict_jianpin(
        &self,
        key: &str,
        segments: &[String],
        syllables: &std::collections::HashSet<String>,
    ) -> bool {
        self.recursive_strict_match(key, segments, syllables)
    }

    fn recursive_strict_match(
        &self,
        key: &str,
        segments: &[String],
        syllables: &std::collections::HashSet<String>,
    ) -> bool {
        // 如果 segments 耗尽，则 key 也必须耗尽（确保音节数一致）
        if segments.is_empty() {
            return key.is_empty();
        }

        if key.is_empty() {
            return false;
        }

        let first_seg = &segments[0];

        // 安全地按字符边界尝试切分
        // 拼音音节最长通常为 6 字节（如 chuang），
        // 但为了 Unicode 安全，我们遍历实际的字符索引
        for (char_count, (byte_idx, _)) in key.char_indices().enumerate() {
            let len = byte_idx;
            if len > 0 && len <= 10 {
                // 适当放宽长度限制以处理带声调的 Unicode
                let syl = &key[..len];
                if syllables.contains(syl) {
                    // 声母必须匹配
                    if syl.starts_with(first_seg) {
                        // 递归匹配剩余音节
                        if self.recursive_strict_match(&key[len..], &segments[1..], syllables) {
                            return true;
                        }
                    }
                }
            }
            if char_count > 8 {
                break;
            } // 一个音节不可能超过 8 个字符
        }

        // 兜底：尝试全量匹配最后一个或唯一一个音节
        if syllables.contains(key) && key.starts_with(first_seg) && segments.len() == 1 {
            return true;
        }

        false
    }

    #[allow(dead_code)]
    pub fn get_random_entry(&self) -> Option<TrieResult<'_>> {
        let len = self.index.len();
        if len == 0 {
            return None;
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        let target_idx = rng.gen_range(0..len);

        let mut stream = self.index.stream();
        let mut current = 0;
        while let Some((_, offset)) = stream.next() {
            if current == target_idx {
                let mut result = None;
                self.read_block(offset as usize, |pair| {
                    if result.is_none() {
                        result = Some(pair);
                    }
                });
                return result;
            }
            current += 1;
        }
        None
    }

    fn read_block<'a>(&'a self, offset: usize, mut f: impl FnMut(TrieResult<'a>)) {
        let data = self.data.as_ref();
        if offset + 4 > data.len() {
            return;
        }

        let count = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap_or([0; 4]));
        let mut cursor = offset + 4;

        for _ in 0..count {
            if cursor + 2 > data.len() {
                break;
            }
            let w_len =
                u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap_or([0; 2])) as usize;
            cursor += 2;
            if cursor + w_len > data.len() {
                break;
            }
            let word = std::str::from_utf8(&data[cursor..cursor + w_len]).unwrap_or("");
            cursor += w_len;

            if cursor + 2 > data.len() {
                break;
            }
            let tr_len =
                u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap_or([0; 2])) as usize;
            cursor += 2;
            if cursor + tr_len > data.len() {
                break;
            }
            let trad = std::str::from_utf8(&data[cursor..cursor + tr_len]).unwrap_or("");
            cursor += tr_len;

            if cursor + 2 > data.len() {
                break;
            }
            let t_len =
                u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap_or([0; 2])) as usize;
            cursor += 2;
            if cursor + t_len > data.len() {
                break;
            }
            let tone = std::str::from_utf8(&data[cursor..cursor + t_len]).unwrap_or("");
            cursor += t_len;

            if cursor + 2 > data.len() {
                break;
            }
            let e_len =
                u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap_or([0; 2])) as usize;
            cursor += 2;
            if cursor + e_len > data.len() {
                break;
            }
            let en = std::str::from_utf8(&data[cursor..cursor + e_len]).unwrap_or("");
            cursor += e_len;

            if cursor + 2 > data.len() {
                break;
            }
            let s_len =
                u16::from_le_bytes(data[cursor..cursor + 2].try_into().unwrap_or([0; 2])) as usize;
            cursor += 2;
            if cursor + s_len > data.len() {
                break;
            }
            let stroke_aux = std::str::from_utf8(&data[cursor..cursor + s_len]).unwrap_or("");
            cursor += s_len;

            if cursor + 4 > data.len() {
                break;
            }
            let weight = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap_or([0; 4]));
            cursor += 4;

            f(TrieResult {
                word,
                trad,
                tone,
                en,
                stroke_aux,
                weight,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_result_clone() {
        let result = TrieResult {
            word: "test",
            trad: "測試",
            tone: "tēst",
            en: "test",
            stroke_aux: "",
            weight: 100,
        };
        let cloned = result;
        assert_eq!(result.word, cloned.word);
        assert_eq!(result.weight, cloned.weight);
    }

    #[test]
    fn test_trie_data_memory() {
        let data = vec![1u8, 2, 3, 4];
        let trie_data = TrieData::Memory(Arc::new(data.clone()));
        assert_eq!(trie_data.as_ref(), &[1, 2, 3, 4]);
    }

    #[test]
    fn test_trie_result_copy() {
        let result = TrieResult {
            word: "hello",
            trad: "hello",
            tone: "",
            en: "hello",
            stroke_aux: "",
            weight: 0,
        };
        assert_eq!(result.word, "hello");
        assert_eq!(result.weight, 0);
    }
}
