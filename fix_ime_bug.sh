#!/bin/bash

# Rust IME Bug Fix Script
# Main issue: uinput kernel module not loaded

echo "=== Rust IME Bug Fix ==="
echo "正在检查和修复 Rust IME 的已知问题..."

# 检查当前用户权限
echo "1. 检查用户权限..."
if groups $USER | grep -q "input"; then
    echo "✓ 用户已在 input 组中"
else
    echo "❌ 用户不在 input 组中，请运行:"
    echo "   sudo usermod -aG input,uinput $USER"
    echo "   然后注销并重新登录"
fi

# 检查 uinput 设备
echo "2. 检查 uinput 设备..."
if [ -e /dev/uinput ]; then
    echo "✓ uinput 设备存在"
else
    echo "❌ uinput 设备不存在"
fi

# 检查 uinput 模块
echo "3. 检查 uinput 内核模块..."
if lsmod | grep -q "uinput"; then
    echo "✓ uinput 模块已加载"
else
    echo "❌ uinput 模块未加载 - 这是主要问题!"
    echo ""
    echo "请运行以下命令修复:"
    echo "   sudo modprobe uinput"
    echo ""
    echo "为了永久解决此问题，请运行:"
    echo "   echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf"
fi

# 检查 Wayland 环境
echo "4. 检查显示环境..."
if [ "$XDG_SESSION_TYPE" = "wayland" ]; then
    echo "✓ 检测到 Wayland 环境"
    echo "  备用方案: ydotool 已安装"
    if command -v ydotool &> /dev/null; then
        echo "  ✓ ydotool 可用"
        # 检查 ydotoold 服务
        if systemctl --user is-active --quiet ydotoold 2>/dev/null; then
            echo "  ✓ ydotoold 服务正在运行"
        else
            echo "  ⚠ ydotoold 服务未运行，可能需要手动启动:"
            echo "    systemctl --user enable --now ydotoold"
        fi
    else
        echo "  ❌ ydotool 未安装，请运行:"
        echo "    sudo apt install ydotool"
    fi
else
    echo "✓ X11 环境，无需额外配置"
fi

# 检查配置文件
echo "5. 检查配置文件..."
CONFIG_DIR="$HOME/.local/share/rust-ime"
PROJECT_CONFIG="./config.json"

if [ -f "$PROJECT_CONFIG" ]; then
    echo "✓ 项目配置文件存在"
elif [ -f "$CONFIG_DIR/config.json" ]; then
    echo "✓ 用户配置文件存在"
else
    echo "⚠ 未找到配置文件，将使用默认配置"
fi

echo ""
echo "=== 修复建议 ==="
echo "1. 首要修复 (必需):"
echo "   sudo modprobe uinput"
echo "   echo 'uinput' | sudo tee /etc/modules-load.d/uinput.conf"
echo ""
echo "2. 重启 Rust IME:"
echo "   rust-ime --stop"
echo "   rust-ime --foreground  # 测试运行"
echo ""
echo "3. 如果仍有问题，请检查日志:"
echo "   tail -f /tmp/rust-ime.log"
echo ""
echo "=== 常见问题解决方案 ==="
echo "• 如果剪贴板不工作 (Wayland):"
echo "  systemctl --user enable --now ydotoold"
echo ""
echo "• 如果权限不足:"
echo "  sudo usermod -aG input,uinput \$USER"
echo "  # 注销并重新登录"
echo ""
echo "• 如果键盘设备找不到:"
echo "  # 检查键盘设备列表:"
echo "  ls -la /dev/input/by-id/"
echo "  # 然后在 config.json 中设置 device_path"