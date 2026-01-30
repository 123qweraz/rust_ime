use fst::{MapBuilder};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use serde_json::Value;
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("data")?;
    compile_dict_for_path("dicts/chinese", "data/chinese")?;
    compile_dict_for_path("dicts/japanese", "data/japanese")?;
    compile_ngram()?;
    Ok(())
}

fn compile_dict_for_path(src_dir: &str, out_stem: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    println!("[Compiler] Compiling dictionary from {} -> {}...", src_dir, out_stem);
    
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            // Skip punctuation file for Trie (it's handled separately)
            if entry.path().file_name().and_then(|n| n.to_str()).map_or(false, |n| n == "punctuation.json") {
                continue;
            }
            process_json_file(entry.path(), &mut entries)?;
        }
    }
    
    let idx_path = format!("{}.index", out_stem);
    let dat_path = format!("{}.data", out_stem);
    write_binary_dict(&idx_path, &dat_path, entries)?;
    println!("[Compiler] Finished: {}", out_stem);
    Ok(())
}

fn process_json_file(path: &Path, entries: &mut BTreeMap<String, Vec<(String, String)>>) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let json: Value = serde_json::from_reader(file)?;
    if let Some(obj) = json.as_object() {
        for (pinyin, val) in obj {
            let pinyin_lower = pinyin.to_lowercase();
            if let Some(arr) = val.as_array() {
                for v in arr {
                    if let Some(s) = v.as_str() { entries.entry(pinyin_lower.clone()).or_default().push((s.to_string(), String::new())); }
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
    Ok(())
}

fn write_binary_dict(idx_path: &str, dat_path: &str, entries: BTreeMap<String, Vec<(String, String)>>) -> Result<(), Box<dyn std::error::Error>> {
    let data_file = File::create(dat_path)?;
    let mut data_writer = BufWriter::new(data_file);
    let mut index_builder = MapBuilder::new(File::create(idx_path)?)?;

    let mut current_offset = 0u64;
    for (pinyin, mut pairs) in entries {
        let mut seen = std::collections::HashSet::new();
        pairs.retain(|(c, _)| seen.insert(c.clone()));

        index_builder.insert(&pinyin, current_offset)?;
        let mut block = Vec::new();
        block.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
        for (word, hint) in pairs {
            let w_bytes = word.as_bytes();
            let h_bytes = hint.as_bytes();
            block.extend_from_slice(&(w_bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(w_bytes);
            block.extend_from_slice(&(h_bytes.len() as u16).to_le_bytes());
            block.extend_from_slice(h_bytes);
        }
        data_writer.write_all(&block)?;
        current_offset += block.len() as u64;
    }
    index_builder.finish()?;
    data_writer.flush()?;
    Ok(())
}

fn compile_ngram() -> Result<(), Box<dyn std::error::Error>> {
    let mut transitions: BTreeMap<String, HashMap<String, u32>> = BTreeMap::new();
    let mut unigrams: BTreeMap<String, u32> = BTreeMap::new();
    for entry in WalkDir::new("n-gram-model").into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().map_or(false, |ext| ext == "json") {
            let file = File::open(entry.path())?;
            let json: Value = serde_json::from_reader(file)?;
            if let Some(obj) = json.as_object() {
                if let Some(trans) = obj.get("transitions").and_then(|t| t.as_object()) {
                    for (ctx, next_map_val) in trans {
                        if let Some(next_map) = next_map_val.as_object() {
                            let entry = transitions.entry(ctx.clone()).or_default();
                            for (token, score) in next_map { if let Some(s) = score.as_u64() { *entry.entry(token.clone()).or_default() += s as u32; } }
                        }
                    }
                }
                if let Some(unis) = obj.get("unigrams").and_then(|u| u.as_object()) {
                    for (token, score) in unis { if let Some(s) = score.as_u64() { *unigrams.entry(token.clone()).or_default() += s as u32; } }
                }
            }
        }
    }
    let mut data_writer = BufWriter::new(File::create("data/ngram.data")?);
    let mut index_builder = MapBuilder::new(File::create("data/ngram.index")?)?;
    let mut unigram_builder = MapBuilder::new(File::create("data/ngram.unigram")?)?;
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
    index_builder.finish()?; data_writer.flush()?;
    for (token, score) in unigrams { unigram_builder.insert(&token, score as u64)?; }
    unigram_builder.finish()?;
    println!("[Compiler] N-gram compiled.");
    Ok(())
}