use rust_embed::Embed;
use std::path::Path;

#[derive(Embed)]
#[folder = "assets/"]
struct Assets;

fn main() {
    let out_dir_var = std::env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_var);
    let _dest_path = out_dir.join("embedded.rs");

    // This will generate the embedded assets
    let _ = Assets::iter().collect::<Vec<_>>();

    println!("cargo:rerun-if-changed=assets/");
}
