use fst::MapBuilder;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use serde_json::Value;
use walkdir::WalkDir;

use std::time::SystemTime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("data")?;

    // 动态扫描 dicts 目录下的所有子目录并编译
    if let Ok(entries) = fs::read_dir("dicts") {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name().to_string_lossy().to_string();
                let src_path = format!("dicts/{}", dir_name);
                let out_dir = format!("data/{}", dir_name);
                fs::create_dir_all(&out_dir)?;
                
                let trie_idx = format!("{}/trie.index", out_dir);
                let local_ngram_src = format!("{}/n-gram-model", src_path);
                
                // 1. 检查是否需要编译 Trie
                if should_compile(Path::new(&src_path), Path::new(&trie_idx)) {
                    compile_dict_for_path(&src_path, &format!("{}/trie", out_dir))?;
                } else {
                    println!("[Compiler] Skipping Trie for: {} (No changes detected)", dir_name);
                }
                
                // 2. 检查并编译 N-gram
                let ngram_idx = format!("{}/ngram.index", out_dir);
                if Path::new(&local_ngram_src).exists() {
                    if should_compile(Path::new(&local_ngram_src), Path::new(&ngram_idx)) {
                        println!("[Compiler] Compiling local N-gram model for: {}", dir_name);
                        compile_ngram_for_path(&local_ngram_src, &out_dir)?;
                    }
                } else if dir_name == "chinese" && Path::new("n-gram-model").exists() {
                    if should_compile(Path::new("n-gram-model"), Path::new(&ngram_idx)) {
                        compile_ngram_for_path("n-gram-model", &out_dir)?;
                    } else {
                        println!("[Compiler] Skipping Chinese N-gram (No changes detected)");
                    }
                }
            }
        }
    }
    
    // 自动提取音节表 (优先从 chinese/chars.json 提取)
    if Path::new("dicts/chinese/chars.json").exists() {
        extract_syllables_to_file("dicts/chinese/chars.json", "dicts/chinese/syllables.txt")?;
    }
    
    Ok(())
}

fn should_compile(src_dir: &Path, target_file: &Path) -> bool {
    if !target_file.exists() { return true; }
    
    let target_mtime = target_file.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    
    // 1. 检查文件夹本身的修改时间 (新增/删除文件会触发)
    if let Ok(dir_mtime) = src_dir.metadata().and_then(|m| m.modified()) {
        if dir_mtime > target_mtime { return true; }
    }

    // 2. 递归检查源目录下所有文件的最大修改时间
    let mut max_src_mtime = SystemTime::UNIX_EPOCH;
    let mut file_count = 0;
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            file_count += 1;
            if let Ok(mtime) = entry.path().metadata().and_then(|m| m.modified()) {
                if mtime > max_src_mtime { max_src_mtime = mtime; }
            }
        }
    }
    
    // 如果内部文件有更新，或者逻辑上我们想更严格一点 (比如记录上一次的文件总数)
    // 这里我们先通过 mtime 判定，通常 dir_mtime 已经能涵盖新增/删除了
    max_src_mtime > target_mtime
}

fn extract_syllables_to_file(src_json: &str, out_txt: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[Compiler] Extracting syllables from {}...", src_json);
    let file = File::open(src_json)?;
    let json: Value = serde_json::from_reader(file)?;
    if let Some(obj) = json.as_object() {
        let mut syllables: Vec<_> = obj.keys().cloned().collect();
        syllables.sort();
        let mut f = File::create(out_txt)?;
        for s in syllables {
            writeln!(f, "{}", s)?;
        }
    }
    Ok(())
}

fn compile_dict_for_path(src_dir: &str, out_stem: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    println!("[Compiler] Compiling dictionary from {} -> {}...", src_dir, out_stem);
    
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            if path.file_name().and_then(|n| n.to_str()).map_or(false, |n| n == "punctuation.json") {
                continue;
            }
            process_json_file(path, &mut entries)?;
        } else if path.extension().map_or(false, |ext| ext == "yaml") {
            process_yaml_file(path, &mut entries)?;
        }
    }
    
    let idx_path = format!("{}.index", out_stem);
    let dat_path = format!("{}.data", out_stem);
    write_binary_dict(&idx_path, &dat_path, entries)?;
    println!("[Compiler] Finished: {}", out_stem);
    Ok(())
}

fn process_yaml_file(path: &Path, entries: &mut BTreeMap<String, Vec<(String, String)>>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, BufReader};
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut in_data = false;

    for line in reader.lines() {
        let line = line?;
        if !in_data {
            if line.starts_with("...") { in_data = true; }
            continue;
        }
        if line.starts_with('#') || line.trim().is_empty() { continue; }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let word = parts[0].to_string();
            let pinyin = parts[1].replace(' ', "").to_lowercase();
            entries.entry(pinyin).or_default().push((word, String::new()));
        }
    }
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

fn compile_ngram_for_path(src_dir: &str, out_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut transitions: BTreeMap<String, HashMap<String, u32>> = BTreeMap::new();
    let mut unigrams: BTreeMap<String, u32> = BTreeMap::new();
    for entry in WalkDir::new(src_dir).into_iter().filter_map(|e| e.ok()) {
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
    
    if transitions.is_empty() && unigrams.is_empty() { return Ok(()); }

    let mut data_writer = BufWriter::new(File::create(format!("{}/ngram.data", out_dir))?);
    let mut index_builder = MapBuilder::new(File::create(format!("{}/ngram.index", out_dir))?)?;
    let mut unigram_builder = MapBuilder::new(File::create(format!("{}/ngram.unigram", out_dir))?)?;
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
    for (token, score) in unigrams { unigram_builder.insert(&token, score as u64)?;
    }
    unigram_builder.finish()?;
    println!("[Compiler] N-gram compiled to: {}", out_dir);
    Ok(())
}
