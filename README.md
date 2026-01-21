# Blind-IME

一个为盲人或无障碍需求设计的 Linux 系统级拼音输入法。它通过直接监听 `/dev/input/event` 设备，并利用虚拟键盘 (uinput) 和系统剪贴板实现高效的汉字输入。

## 快速开始 (其他电脑部署)

如果你想在新的电脑上运行此项目，请按照以下步骤操作：

### 1. 安装系统依赖

编译和运行需要一些底层库（主要用于剪贴板支持）：

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install libxcb-composite0-dev libx11-dev
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

- **系统级监听**：在底层拦截按键，支持全局使用。
- **剪贴板加速**：通过剪贴板 + `Ctrl+V` 实现瞬间上屏，避免 Unicode 序列的输入延迟。
- **无障碍友好**：支持通过 `Shift` 键快速切换中英文模式。
- **灵活词库**：支持多级 JSON 词库加载，可自定义扩展。

## 配置说明

通过修改 `config.json` 调整词库路径：

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
