use fst::{MapBuilder};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use serde_json::Value;
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    
    println!("[Compiler] Reading JSON dictionaries...");
    for entry in WalkDir::new("dicts").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            if let Some(obj) = json.as_object() {
                for (pinyin, val) in obj {
                    let pinyin_lower = pinyin.to_lowercase();
                    let words = if let Some(arr) = val.as_array() {
                        arr.iter().filter_map(|v| {
                            if v.is_string() { Some(v.as_str().unwrap().to_string()) }
                            else if v.is_object() { Some(v["char"].as_str().unwrap().to_string()) }
                            else { None }
                        }).collect::<Vec<String>>()
                    } else { vec![] };
                    
                    entries.entry(pinyin_lower).or_default().extend(words);
                }
            }
        }
    }

    // 写入数据文件和构建索引
    let data_file = File::create("dict.data")?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create("dict.index")?)?;

    let mut current_offset = 0u64;
    println!("[Compiler] Compiling {} entries into binary format...", entries.len());

    for (pinyin, mut words) in entries {
        // 去重
        words.sort();
        words.dedup();

        // 记录索引：拼音 -> 当前数据偏移量
        index_builder.insert(&pinyin, current_offset)?;

        // 写入数据块
        let mut block = Vec::new();
        // 词数 (u32)
        block.extend_from_slice(&(words.len() as u32).to_le_bytes());
        for word in words {
            let bytes = word.as_bytes();
            // 词长 (u16)
            block.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            // 词内容
            block.extend_from_slice(bytes);
        }

        data_writer.write_all(&block)?;
        current_offset += block.len() as u64;
    }

    index_builder.finish()?;
    data_writer.flush()?;

    println!("[Compiler] Done! dict.index and dict.data created.");
    Ok(())
}
