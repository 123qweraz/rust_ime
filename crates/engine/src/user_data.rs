use crate::config_manager::UserDictData;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const DATA_VERSION: &str = "1.0";

#[derive(Clone, Debug)]
pub enum DataType {
    Learned,
    Usage,
    Ngram,
}

impl DataType {
    fn filename(&self) -> &str {
        match self {
            DataType::Learned => "learned.json",
            DataType::Usage => "usage.json",
            DataType::Ngram => "ngrams.json",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JsonDataFile {
    version: String,
    updated_at: Option<String>,
    data: HashMap<String, Vec<(String, u32)>>,
}

pub struct UserDataManager {
    data_dir: PathBuf,
    dirty: Arc<AtomicBool>,
}

impl UserDataManager {
    pub fn new(data_dir: PathBuf) -> std::io::Result<Self> {
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
        }
        Ok(Self {
            data_dir,
            dirty: Arc::new(AtomicBool::new(false)),
        })
    }

    fn profile_dir(&self, profile: &str) -> PathBuf {
        self.data_dir.join(profile)
    }

    fn ensure_profile_dir(&self, profile: &str) -> std::io::Result<PathBuf> {
        let dir = self.profile_dir(profile);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    fn timestamp() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    }

    pub fn load(&self, profile: &str, data_type: DataType) -> HashMap<String, Vec<(String, u32)>> {
        let file_path = self.profile_dir(profile).join(data_type.filename());

        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(data_file) = serde_json::from_str::<JsonDataFile>(&content) {
                    return data_file.data;
                }
                if let Ok(data) =
                    serde_json::from_str::<HashMap<String, Vec<(String, u32)>>>(&content)
                {
                    return data;
                }
            }
        }

        HashMap::new()
    }

    pub fn load_all(&self, profiles: &[String]) -> (UserDictData, UserDictData, UserDictData) {
        let mut learned: UserDictData = UserDictData::new();
        let mut usage: UserDictData = UserDictData::new();
        let mut ngrams: UserDictData = UserDictData::new();

        for profile in profiles {
            let dir = self.profile_dir(profile);
            if dir.exists() {
                let l = self.load(profile, DataType::Learned);
                let u = self.load(profile, DataType::Usage);
                let n = self.load(profile, DataType::Ngram);

                if !l.is_empty() {
                    learned.insert(profile.clone(), l);
                }
                if !u.is_empty() {
                    usage.insert(profile.clone(), u);
                }
                if !n.is_empty() {
                    ngrams.insert(profile.clone(), n);
                }
            }
        }

        if learned.is_empty() {
            Self::load_from_legacy_json_static(&mut learned, DataType::Learned);
        }
        if usage.is_empty() {
            Self::load_from_legacy_json_static(&mut usage, DataType::Usage);
        }

        (learned, usage, ngrams)
    }

    fn load_from_legacy_json_static(data: &mut UserDictData, data_type: DataType) {
        let legacy_file = match data_type {
            DataType::Learned => Path::new("data/learned_words.json"),
            DataType::Usage => Path::new("data/usage_history.json"),
            DataType::Ngram => return,
        };

        if legacy_file.exists() {
            if let Ok(content) = fs::read_to_string(legacy_file) {
                if let Ok(loaded) = serde_json::from_str::<UserDictData>(&content) {
                    data.extend(loaded);
                }
            }
        }
    }

    pub fn save(
        &self,
        profile: &str,
        data_type: DataType,
        data: &HashMap<String, Vec<(String, u32)>>,
    ) -> std::io::Result<()> {
        let dir = self.ensure_profile_dir(profile)?;
        let file_path = dir.join(data_type.filename());

        let data_file = JsonDataFile {
            version: DATA_VERSION.to_string(),
            updated_at: Some(Self::timestamp()),
            data: data.clone(),
        };

        let json = serde_json::to_string_pretty(&data_file)?;
        fs::write(&file_path, json)?;

        self.dirty.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn save_user_dict(
        &self,
        profile: &str,
        data_type: DataType,
        data: &UserDictData,
    ) -> std::io::Result<()> {
        if let Some(profile_data) = data.get(profile) {
            self.save(profile, data_type, profile_data)?;
        }
        Ok(())
    }

    pub fn list_profiles(&self) -> Vec<String> {
        let mut profiles = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.data_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                        profiles.push(name.to_string());
                    }
                }
            }
        }
        profiles
    }

    pub fn clear(&self, profile: &str, data_type: Option<DataType>) -> std::io::Result<()> {
        match data_type {
            Some(dt) => {
                let empty: HashMap<String, Vec<(String, u32)>> = HashMap::new();
                self.save(profile, dt, &empty)?;
            }
            None => {
                for dt in &[DataType::Learned, DataType::Usage, DataType::Ngram] {
                    let empty: HashMap<String, Vec<(String, u32)>> = HashMap::new();
                    self.save(profile, dt.clone(), &empty)?;
                }
            }
        }
        Ok(())
    }
}
