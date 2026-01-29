use fst::{Map, IntoStreamer, Streamer, Automaton};
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct MmapData(Arc<Mmap>);

impl AsRef<[u8]> for MmapData {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
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
        
        let index_mmap = unsafe { Mmap::map(&index_file)? };
        let data_mmap = unsafe { Mmap::map(&data_file)? };
        
        let index_data = MmapData(Arc::new(index_mmap));
        let data_data = MmapData(Arc::new(data_mmap));
        
        let index = Map::new(index_data)?;
        
        Ok(Self {
            index,
            data: data_data,
        })
    }

    pub fn get_all_exact(&self, pinyin: &str) -> Option<Vec<String>> {
        let offset = self.index.get(pinyin)? as usize;
        Some(self.read_block(offset))
    }

    pub fn search_bfs(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut results = Vec::new();
        
        let matcher = fst::automaton::Str::new(prefix).starts_with();
        let mut stream = self.index.search(matcher).into_stream();

        while let Some((_, offset)) = stream.next() {
            let words = self.read_block(offset as usize);
            for word in words {
                if !results.contains(&word) {
                    results.push(word);
                    if results.len() >= limit {
                        return results;
                    }
                }
            }
        }
        
        results
    }

    fn read_block(&self, offset: usize) -> Vec<String> {
        let data = self.data.as_ref();
        let mut cursor = offset;
        
        let count_bytes = &data[cursor..cursor+4];
        let count = u32::from_le_bytes(count_bytes.try_into().unwrap());
        cursor += 4;
        
        let mut words = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let len_bytes = &data[cursor..cursor+2];
            let len = u16::from_le_bytes(len_bytes.try_into().unwrap()) as usize;
            cursor += 2;
            
            let word_bytes = &data[cursor..cursor+len];
            let word = String::from_utf8_lossy(word_bytes).to_string();
            words.push(word);
            cursor += len;
        }
        
        words
    }
}
