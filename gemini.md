# Gemini CLI - Change Log & Instructions

## Instructions to Run

Since this program accesses input devices (`/dev/input/`), it requires root privileges.

### Option 1: Using cargo with user PATH (Recommended)
```bash
sudo env "PATH=$PATH" cargo run
```

### Option 2: Build and run the binary directly
```bash
cargo build
sudo ./target/debug/blind-ime
```

### Enable Level-3 Dictionary
```bash
sudo env "PATH=$PATH" cargo run -- --level3
```

---

## Change Log

### 2026-01-21: Dictionary Optimization
- **Priority Loading:** Modified `load_dict` to load `level-1_char_en.json` first, followed by `level-2_char_en.json`.
- **Level-3 Control:** `level-3_char_en.json` is now disabled by default. It can be enabled using the `--level3` command-line flag.
- **Candidate Ordering:** Improved `load_file_into_dict` to ensure that higher priority characters appear first in the candidate list and prevented duplicate candidates for the same pinyin.
- **Refactoring:** Extracted dictionary loading logic into a helper function for better maintainability.
