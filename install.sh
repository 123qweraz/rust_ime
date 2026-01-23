#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

echo "=== Blind-IME 自动安装脚本 ==="

# 0. Check Rust environment
if ! command -v cargo &> /dev/null; then
    echo "❌ 错误: 未检测到 Rust/Cargo 环境"
    echo "请先安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 1. Install Dependencies
echo -e "\n[1/4] 安装系统依赖..."
if [ -f /etc/debian_version ]; then
    # Detect Debian/Ubuntu/Pop!_OS
    echo "检测到 Debian 系系统，正在使用 apt 安装依赖..."
    sudo apt-get update
    sudo apt-get install -y libxcb-composite0-dev libx11-dev libdbus-1-dev build-essential
else
    echo "⚠️  未检测到 apt 包管理器"
    echo "请确保已手动安装以下开发库："
    echo "  - libxcb-composite0-dev"
    echo "  - libx11-dev"
    echo "  - libdbus-1-dev"
    read -p "按回车键继续..."
fi

# 2. Configure Permissions
echo -e "\n[2/4] 配置用户权限..."
CURRENT_USER=$(whoami)

# Add to input group
if groups | grep -q "\binput\b"; then
    echo "✅ 用户 '$CURRENT_USER' 已经在 input 组中"
else
    echo "正在将用户 '$CURRENT_USER' 加入 input 组..."
    sudo usermod -aG input "$CURRENT_USER"
    echo "✅ 已添加 (需要注销后生效)"
fi

# Udev rules for uinput
echo "正在配置 uinput 设备规则..."
if [ ! -f /etc/udev/rules.d/99-blind-ime-uinput.rules ]; then
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-blind-ime-uinput.rules > /dev/null
    echo "✅ 规则文件已创建"
    sudo udevadm control --reload-rules
    sudo udevadm trigger
else
    echo "✅ 规则文件已存在"
fi

# 3. Build Project
echo -e "\n[3/4] 正在编译项目 (Release模式)..."
cargo build --release

# 4. Install Autostart
echo -e "\n[4/4] 配置开机自启..."
./target/release/blind-ime --install

echo -e "\n=========================================="
echo "🎉 安装完成！"
echo "⚠️  注意: 如果是第一次运行脚本并被添加到了 input 组，"
echo "    你必须【注销并重新登录】(或重启电脑) 才能正常使用！"
echo ""
echo "手动启动命令: ./target/release/blind-ime"
echo "==========================================