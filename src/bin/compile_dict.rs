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
    // pinyin -> Vec<(char, hint)>
    let mut entries: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    
    println!("[Compiler] Scanning dicts directory...");
    for entry in WalkDir::new("dicts").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            println!("[Compiler] Processing {}", entry.path().display());
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            
            if let Some(obj) = json.as_object() {
                for (pinyin, val) in obj {
                    let pinyin_lower = pinyin.to_lowercase();
                    
                    if let Some(arr) = val.as_array() {
                        for v in arr {
                            if let Some(s) = v.as_str() { 
                                entries.entry(pinyin_lower.clone()).or_default().push((s.to_string(), String::new())); 
                            }
                            else if let Some(o) = v.as_object() {
                                if let Some(c) = o.get("char").and_then(|c| c.as_str()) {
                                    let hint = o.get("en").and_then(|e| e.as_str()).unwrap_or("").to_string();
                                    entries.entry(pinyin_lower.clone()).or_default().push((c.to_string(), hint));
                                }
                            }
                        }
                    } else if let Some(s) = val.as_str() {
                        entries.entry(pinyin_lower).or_default().push((s.to_string(), String::new()));
                    }
                }
            }
        }
    }

    let data_file = File::create("dict.data")?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create("dict.index")?)?;

    let mut current_offset = 0u64;
    for (pinyin, mut pairs) in entries {
        // 去重并保持顺序
        let mut seen = std::collections::HashSet::new();
        pairs.retain(|(c, _)| seen.insert(c.clone()));

        index_builder.insert(&pinyin, current_offset)?;
        let mut block = Vec::new();
        block.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
        for (word, hint) in pairs {
            let w_bytes = word.as_bytes();
            let h_bytes = hint.as_bytes();
            // 写入词
            block.extend_from_slice(&(w_bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(w_bytes);
            // 写入提示
            block.extend_from_slice(&(h_bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(h_bytes);
        }
        data_writer.write_all(&block)?;
        current_offset += block.len() as u64;
    }
    index_builder.finish()?;
    data_writer.flush()?;
    println!("[Compiler] Dictionary compiled with hints.");
    Ok(())
}

fn compile_ngram() -> Result<(), Box<dyn std::error::Error>> {
    let mut transitions: BTreeMap<String, HashMap<String, u32>> = BTreeMap::new();
    let mut unigrams: BTreeMap<String, u32> = BTreeMap::new();

    println!("[Compiler] Scanning n-gram-model directory...");
    for entry in WalkDir::new("n-gram-model").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            if let Some(obj) = json.as_object() {
                if obj.contains_key("transitions") {
                    if let Some(trans) = obj["transitions"].as_object() {
                        for (ctx, next_map_val) in trans {
                            if let Some(next_map) = next_map_val.as_object() {
                                let entry = transitions.entry(ctx.clone()).or_default();
                                for (token, score) in next_map {
                                    if let Some(s) = score.as_u64() { *entry.entry(token.clone()).or_default() += s as u32; }
                                }
                            }
                        }
                    }
                    if let Some(unis) = obj["unigrams"].as_object() {
                        for (token, score) in unis {
                            if let Some(s) = score.as_u64() { *unigrams.entry(token.clone()).or_default() += s as u32; }
                        }
                    }
                }
            }
        }
    }

    let data_file = File::create("ngram.data")?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create("ngram.index")?)?;
    let mut unigram_builder = MapBuilder::new(File::create("ngram.unigram")?)?;

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

    for (token, score) in unigrams { unigram_builder.insert(&token, score as u64)?; }
    unigram_builder.finish()?;
    println!("[Compiler] N-gram compiled.");
    Ok(())
}
