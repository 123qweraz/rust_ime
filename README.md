# Blind-IME

一个为盲人或无障碍需求设计的系统级拼音输入法。它通过直接监听 `/dev/input/event` 设备，并利用虚拟键盘（uinput）和系统剪贴板实现高效的汉字输入。

## 核心特性

- **系统级监听**：在底层拦截按键，支持全局使用。
- **剪贴板加速**：通过剪贴板 + `Ctrl+V` 实现瞬间上屏，避免 Unicode 序列的输入延迟。
- **无障碍友好**：设计简洁，支持通过 `Shift` 键快速切换中英文模式。
- **灵活词库**：支持多级 JSON 词库加载，可自定义扩展。
- **前缀匹配**：支持简拼及前缀搜索，输入 `ni` 即可匹配“你好”、“逆转”等。

## 安装要求

- **操作系统**：Linux
- **权限**：需要读写 `/dev/input/` 和 `/dev/uinput` 的权限（通常需要 `sudo` 或加入 `input` 组）。
- **依赖库**：
  ```bash
  sudo apt-get install libxcb-composite0-dev libx11-dev
  ```

## 快速开始

1. **克隆仓库**：
   ```bash
   git clone https://github.com/your-username/blind-ime.git
   cd blind-ime
   ```

2. **编译**：
   ```bash
   cargo build --release
   ```

3. **运行**：
   ```bash
   sudo ./target/release/blind-ime
   ```

## 配置说明

你可以通过修改 `config.json` 来调整词库路径：

```json
{
  "dict_dirs": ["dicts/chinese/character", "dicts/chinese/vocabulary"],
  "extra_dicts": ["dicts/dict_new.json"],
  "enable_level3": false
}
```

## 按键说明

- **Shift**：切换中英文模式。
- **字母键**：输入拼音。
- **空格 (Space)**：确认首个候选词上屏。
- **数字 (1-9)**：选择对应的候选词上屏。
- **Tab**：在候选词之间循环切换。
- **回车 (Enter)**：直接发送当前的拼音 Buffer。
- **退格 (Backspace)**：删除拼音字符。

## 许可证

MIT
