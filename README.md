# Blind-IME

一个为盲人或无障碍需求设计的 Linux 系统级拼音输入法。它通过直接监听 `/dev/input/event` 设备，并利用虚拟键盘 (uinput) 和系统剪贴板实现高效的汉字输入。

## 快速开始 (其他电脑部署)

如果你想在新的电脑上运行此项目，请按照以下步骤操作：

### 1. 安装系统依赖

编译和运行需要一些底层库（用于剪贴板支持和系统通知）：

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install libxcb-composite0-dev libx11-dev libdbus-1-dev
```

### 2. 配置权限 (重要)

由于输入法需要直接读取键盘事件并模拟按键，必须授予当前用户访问输入设备的权限，否则必须使用 `sudo` 运行。

**按照以下步骤免 `sudo` 运行：**

1.  **加入 `input` 组**：
    将当前用户添加到 `input` 用户组，以便读取原始按键：
    ```bash
    sudo usermod -aG input $USER
    ```

2.  **配置 `uinput` 权限**：
    创建 udev 规则，允许 `input` 组写入虚拟键盘设备：
    ```bash
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-uinput.rules
    ```

3.  **重新登录**：
    执行完上述命令后，**必须注销并重新登录**（或重启电脑）才能使组权限生效。

### 3. 编译项目

确保你已经安装了 [Rust 环境](https://rustup.rs/)：

```bash
git clone https://github.com/your-username/blind-ime.git
cd blind-ime
cargo build --release
```

### 4. 运行

```bash
# 如果已经配置了上述权限并重启过：
./target/release/blind-ime

# 如果没有配置权限，则需要 sudo：
sudo ./target/release/blind-ime
```

## 核心特性

- **系统级监听**：在底层拦截按键，支持全局使用 (Wayland/X11/Console)。
- **幽灵文字 (Phantom Text)**：支持行内实时预览。输入拼音时，首选词直接显示在输入框，按 Tab 切换时实时更新，零延迟上屏。
- **语义辅助码 (Semantic Auxiliary)**：支持通过英文单词筛选汉字。例如输入 `li` 后按 `Shift+I` (Inside)，即可快速定位到“里”。
- **剪贴板加速**：通过剪贴板 + `Ctrl+V` 实现瞬间上屏，避免 Unicode 序列的输入延迟。
- **无障碍友好**：支持通过 `Shift` 键快速切换中英文模式。

## 快捷键说明

| 快捷键 | 功能 | 说明 |
| :--- | :--- | :--- |
| **Ctrl + Space** | 切换中/英文模式 | 开启或关闭输入法 |
| **Ctrl + Alt + P** | 切换幽灵文字模式 | 开启后候选词直接显示在光标处 (推荐开启) |
| **Ctrl + Alt + V** | 切换粘贴模式 | 适配不同终端 (Std Ctrl+V / Term Ctrl+Shift+V / Legacy Shift+Ins / Hex) |
| **Tab** | 切换候选词 | 选中下一个候选词 |
| **- / =** | 翻页 | 上一页 / 下一页 |
| **Shift + [A-Z]** | 激活辅助码 | 在拼音后输入大写字母，利用英文释义筛选汉字 |

## 辅助码使用示例

1. **输入拼音**：输入 `li`。
2. **触发筛选**：你想要“里”，联想到英文 **I**nside。
3. **输入辅助码**：按住 Shift 输入 `I`。
4. **结果**：候选词列表自动过滤，只显示英文释义以 "I" 开头的字（如 `1.里(inside)`）。
5. **上屏**：按空格上屏。

## 配置说明

通过修改 `config.json` 调整词库路径：

```json
{
  "dict_dirs": ["dicts/chinese/character", "dicts/chinese/vocabulary"],
  "extra_dicts": ["dicts/dict_new.json"],
  "enable_level3": false
}
```

## Usage

1. Run the program:
   ```bash
   # Foreground mode (for testing)
   sudo ./target/release/blind-ime --foreground

   # Background mode (recommended)
   ./target/release/blind-ime
   ```

2. Toggle Chinese mode: Press **Ctrl + Space**.
3. Type pinyin to see candidates in the terminal.
4. Select words using:
   - `Space`: First candidate
   - `Numbers 1-9`: Specific candidate
   - `Enter`: Raw pinyin

## Service Management (Auto-start & Background)

Blind IME has built-in service management features.

**1. Install Auto-start (Run once)**
To make Blind IME start automatically when you log in:
```bash
./target/release/blind-ime --install
```

**2. Stop the Service**
To stop the background process:
```bash
./target/release/blind-ime --stop
```

**3. View Logs**
If you encounter issues, check the log file:
```bash
cat /tmp/blind-ime.log
```

## 许可证

MIT
