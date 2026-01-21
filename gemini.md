# Gemini CLI - Change Log & Instructions

## Instructions to Run

Since this program accesses input devices (`/dev/input/`), it requires root privileges.

### Option 1: Run the compiled binary directly (Recommended)
This is the most reliable method if `cargo` is not in the root path.
```bash
# First, build the project as a normal user
cargo build

# Then, run the binary with sudo
sudo ./target/debug/blind-ime
```

### Option 2: Using cargo with user PATH
```bash
sudo env "PATH=$PATH" cargo run
```

### Enable Level-3 Dictionary
```bash
sudo ./target/debug/blind-ime --level3
```

---

## Change Log

### 2026-01-21: Selection & Paging Features
- **Numeric Selection:** Press `1-9` (or `0`) to select candidates when typing pinyin.
- **Paging:** Use `-` (minus) for the previous page and `=` (equal) for the next page.
- **Enhanced Shift:** 
    - When typing: Commits the first candidate (English assisted selection logic).
    - When idle: Toggles Chinese/English mode.
- **Enter Key:** Commits the raw pinyin string directly.
- **Internal Logic:** Updated `ImeState` to manage candidate lists and page indexes.
