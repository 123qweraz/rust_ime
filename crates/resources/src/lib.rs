use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/"]
pub struct Assets;

pub mod dicts {
    use super::Assets;
    use std::borrow::Cow;

    pub fn get_syllables(lang: &str) -> Option<Cow<'static, [u8]>> {
        let path = format!("dicts/{}/syllables.txt", lang);
        Assets::get(&path).map(|f| f.data)
    }

    pub fn get_dict_file(lang: &str, name: &str) -> Option<std::path::PathBuf> {
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let candidates = [
            base.join(format!("dicts/{}/{}", lang, name)),
            base.join(format!("dicts/{}", name)),
            base.join(name),
        ];

        candidates.into_iter().find(|p| p.exists())
    }
}

pub mod configs {
    use super::Assets;
    use std::borrow::Cow;

    pub fn get_config(name: &str) -> Option<Cow<'static, [u8]>> {
        let path = format!("configs/{}.json", name);
        Assets::get(&path).map(|f| f.data)
    }
}

pub mod fonts {
    pub fn get_font_path(name: &str) -> Option<std::path::PathBuf> {
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let candidates = [
            base.join(format!("fonts/{}", name)),
            std::path::PathBuf::from(format!("/usr/share/fonts/{}", name)),
        ];

        candidates.into_iter().find(|p| p.exists())
    }
}
