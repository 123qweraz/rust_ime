use fst::{Map, IntoStreamer, Streamer, Automaton};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct MmapData(Arc<Mmap>);
impl AsRef<[u8]> for MmapData {
    fn as_ref(&self) -> &[u8] { self.0.as_ref() }
}

#[derive(Clone)]
pub struct Trie {
    index: Map<MmapData>,
    data: MmapData,
}

impl Trie {
    pub fn load<P: AsRef<Path>>(index_path: P, data_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let index_file = File::open(index_path)?;
        let data_file = File::open(data_path)?;
        let index_data = MmapData(Arc::new(unsafe { Mmap::map(&index_file)? }));
        let data_data = MmapData(Arc::new(unsafe { Mmap::map(&data_file)? }));
        let index = Map::new(index_data)?;
        Ok(Self { index, data: data_data })
    }

    pub fn get_all_exact(&self, pinyin: &str) -> Option<Vec<(String, String)>> {
        let offset = self.index.get(pinyin)? as usize;
        Some(self.read_block(offset))
    }

    pub fn search_bfs(&self, prefix: &str, limit: usize) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let matcher = fst::automaton::Str::new(prefix).starts_with();
        let mut stream = self.index.search(matcher).into_stream();

        while let Some((_, offset)) = stream.next() {
            let pairs = self.read_block(offset as usize);
            for pair in pairs {
                if !results.iter().any(|(w, _)| w == &pair.0) {
                    results.push(pair);
                    if results.len() >= limit { return results; }
                }
            }
        }
        results
    }

    pub fn get_random_entry(&self) -> Option<(String, String)> {
        let len = self.index.len();
        if len == 0 { return None; }
        
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let target_idx = rng.gen_range(0..len);
        
        let mut stream = self.index.stream();
        let mut current = 0;
        while let Some((_, offset)) = stream.next() {
            if current == target_idx {
                let pairs = self.read_block(offset as usize);
                return pairs.first().cloned();
            }
            current += 1;
        }
        None
    }

    fn read_block(&self, offset: usize) -> Vec<(String, String)> {
        let data = self.data.as_ref();
        let mut cursor = offset;
        
        let count = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
        cursor += 4;
        
        let mut results = Vec::with_capacity(count as usize);
        for _ in 0..count {
            // 读取词
            let w_len = u16::from_le_bytes(data[cursor..cursor+2].try_into().unwrap()) as usize;
            cursor += 2;
            let word = String::from_utf8_lossy(&data[cursor..cursor+w_len]).to_string();
            cursor += w_len;
            
            // 读取提示
            let h_len = u16::from_le_bytes(data[cursor..cursor+2].try_into().unwrap()) as usize;
            cursor += 2;
            let hint = String::from_utf8_lossy(&data[cursor..cursor+h_len]).to_string();
            cursor += h_len;
            
            results.push((word, hint));
        }
        results
    }
}