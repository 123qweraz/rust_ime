use fst::{MapBuilder};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufWriter, Write};
use serde_json::Value;
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    compile_dictionary()?;
    compile_ngram()?;
    Ok(())
}

fn compile_dictionary() -> Result<(), Box<dyn std::error::Error>> {
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    
    println!("[Compiler] Scanning dicts directory...");
    for entry in WalkDir::new("dicts").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            println!("[Compiler] Processing {}", entry.path().display());
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            
            if let Some(obj) = json.as_object() {
                for (pinyin, val) in obj {
                    let pinyin_lower = pinyin.to_lowercase();
                    let mut words = Vec::new();
                    
                    if let Some(arr) = val.as_array() {
                        for v in arr {
                            if let Some(s) = v.as_str() { words.push(s.to_string()); }
                            else if let Some(o) = v.as_object() {
                                if let Some(c) = o.get("char").and_then(|c| c.as_str()) { words.push(c.to_string()); }
                            }
                        }
                    } else if let Some(s) = val.as_str() { words.push(s.to_string()); }
                    
                    if !words.is_empty() {
                        entries.entry(pinyin_lower).or_default().extend(words);
                    }
                }
            }
        }
    }

    let data_file = File::create("dict.data")?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create("dict.index")?)?;

    let mut current_offset = 0u64;
    for (pinyin, mut words) in entries {
        words.sort(); words.dedup();
        index_builder.insert(&pinyin, current_offset)?;
        let mut block = Vec::new();
        block.extend_from_slice(&(words.len() as u32).to_le_bytes());
        for word in words {
            let bytes = word.as_bytes();
            block.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(bytes);
        }
        data_writer.write_all(&block)?;
        current_offset += block.len() as u64;
    }
    index_builder.finish()?;
    data_writer.flush()?;
    println!("[Compiler] Dictionary compiled.");
    Ok(())
}

fn compile_ngram() -> Result<(), Box<dyn std::error::Error>> {
    // context -> { next_token -> score }
    let mut transitions: BTreeMap<String, HashMap<String, u32>> = BTreeMap::new();
    let mut unigrams: BTreeMap<String, u32> = BTreeMap::new();

    println!("[Compiler] Scanning n-gram-model directory...");
    for entry in WalkDir::new("n-gram-model").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            println!("[Compiler] Processing N-gram file: {}", entry.path().display());
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            
            if let Some(obj) = json.as_object() {
                // 如果是 transitions 结构 (context -> map)
                if obj.contains_key("transitions") {
                    if let Some(trans) = obj["transitions"].as_object() {
                        for (ctx, next_map_val) in trans {
                            if let Some(next_map) = next_map_val.as_object() {
                                let entry = transitions.entry(ctx.clone()).or_default();
                                for (token, score) in next_map {
                                    if let Some(s) = score.as_u64() {
                                        *entry.entry(token.clone()).or_default() += s as u32;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(unis) = obj["unigrams"].as_object() {
                        for (token, score) in unis {
                            if let Some(s) = score.as_u64() {
                                *unigrams.entry(token.clone()).or_default() += s as u32;
                            }
                        }
                    }
                } else {
                    // 兼容直接是 context -> next_map 的结构
                    for (ctx, next_map_val) in obj {
                        if let Some(next_map) = next_map_val.as_object() {
                            let entry = transitions.entry(ctx.clone()).or_default();
                            for (token, score) in next_map {
                                if let Some(s) = score.as_u64() {
                                    *entry.entry(token.clone()).or_default() += s as u32;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 写入 N-gram 二进制
    let data_file = File::create("ngram.data")?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create("ngram.index")?)?;
    let mut unigram_builder = MapBuilder::new(File::create("ngram.unigram")?)?;

    println!("[Compiler] Building N-gram index...");
    let mut current_offset = 0u64;
    for (ctx, next_tokens) in transitions {
        index_builder.insert(&ctx, current_offset)?;
        
        let mut block = Vec::new();
        block.extend_from_slice(&(next_tokens.len() as u32).to_le_bytes());
        for (token, score) in next_tokens {
            let bytes = token.as_bytes();
            block.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(bytes);
            block.extend_from_slice(&score.to_le_bytes());
        }
        data_writer.write_all(&block)?;
        current_offset += block.len() as u64;
    }
    index_builder.finish()?;
    data_writer.flush()?;

    println!("[Compiler] Building Unigram index...");
    for (token, score) in unigrams {
        unigram_builder.insert(&token, score as u64)?;
    }
    unigram_builder.finish()?;

    println!("[Compiler] N-gram compilation success.");
    Ok(())
}