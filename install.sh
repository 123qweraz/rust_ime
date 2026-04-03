#!/bin/bash

# Exit immediately if a command exits with a non-zero status
set -e

echo "=== Rust-IME Auto Installer ==="

# Check if running with precompiled binary
HAS_PRECOMPILED=false
if [ -f "./rust-ime" ]; then
    HAS_PRECOMPILED=true
fi

# 1. Install Dependencies (if needed)
echo -e "\n[1/4] Checking system dependencies..."

if [ "$HAS_PRECOMPILED" = true ]; then
    echo "✅ Precompiled binary detected. Skipping build dependencies."
    echo "   Runtime dependencies (dbus) are usually pre-installed on desktop environments."
else
    echo "⚠️  No precompiled binary found. Will need to build from source."
    
    # Check Rust environment
    if ! command -v cargo &> /dev/null; then
        echo "❌ Error: Rust/Cargo environment not found"
        echo "Please install Rust first: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # Install build dependencies
    echo "Installing build dependencies..."
    if [ -f /etc/debian_version ]; then
        echo "Debian-based system detected, installing via apt..."
        sudo apt-get update
        sudo apt-get install -y build-essential pkg-config clang libdbus-1-dev
    elif [ -f /etc/arch-release ]; then
        echo "Arch-based system detected, installing via pacman..."
        sudo pacman -S --noconfirm --needed base-devel pkgconf clang
    else
        echo "⚠️  Unknown package manager (not apt or pacman)"
        echo "Please ensure the following are installed:"
        echo "  - build-essential, pkg-config, clang"
        echo "  - dbus development files"
        read -p "Press Enter to continue..."
    fi
fi

# 2. Configure Permissions
echo -e "\n[2/4] Configuring user permissions..."
CURRENT_USER=$(whoami)

# Add to input group
if groups | grep -q "\binput\b"; then
    echo "✅ User '$CURRENT_USER' is already in the 'input' group"
else
    echo "Adding user '$CURRENT_USER' to 'input' group..."
    sudo usermod -aG input "$CURRENT_USER"
    echo "✅ Added (Logout/login required to take effect)"
fi

# Udev rules for uinput
echo "Configuring uinput device rules..."
if [ ! -f /etc/udev/rules.d/99-rust-ime-uinput.rules ]; then
    echo 'KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"' | sudo tee /etc/udev/rules.d/99-rust-ime-uinput.rules > /dev/null
    echo "✅ Rule file created"
    sudo udevadm control --reload-rules
    sudo udevadm trigger
else
    echo "✅ Rule file already exists"
fi

# 3. Build Project (if needed)
echo -e "\n[3/4] Preparing program files..."
if [ "$HAS_PRECOMPILED" = true ]; then
    chmod +x ./rust-ime
    echo "✅ Using precompiled binary."
else
    echo "🔨 Building from source (this may take a few minutes)..."
    cargo build --release
    cp target/release/rust-ime .
    echo "✅ Build complete."
fi

# Check for required data files
if [ ! -d "./data" ] || [ -z "$(ls -A ./data 2>/dev/null)" ]; then
    echo "⚠️  Warning: Dictionaries not found. Please ensure 'data' directory exists."
    echo "   You may need to run: ./rust-ime --compile-only"
fi

# 4. Install
echo -e "\n[4/4] Executing installation..."
# Get absolute path
INSTALL_PATH=$(pwd)

# 4.1 Install binary
sudo cp -f "$INSTALL_PATH/rust-ime" /usr/local/bin/rust-ime
sudo chmod +x /usr/local/bin/rust-ime
echo "✅ Installed binary to: /usr/local/bin/rust-ime"

# 4.2 Install icon
ICON_DIR="/usr/share/icons/hicolor/256x256/apps"
sudo mkdir -p "$ICON_DIR"
if [ -f "$INSTALL_PATH/picture/rust-ime_v2.png" ]; then
    sudo cp -f "$INSTALL_PATH/picture/rust-ime_v2.png" "$ICON_DIR/rust-ime.png"
    echo "✅ Installed icon to: $ICON_DIR/rust-ime.png"
fi

# 4.3 Install desktop entry
APP_DIR="/usr/share/applications"
if [ -f "$INSTALL_PATH/rust-ime.desktop" ]; then
    sudo cp -f "$INSTALL_PATH/rust-ime.desktop" "$APP_DIR/rust-ime.desktop"
    sudo update-desktop-database "$APP_DIR" || true
    echo "✅ Installed desktop shortcut. You can now find Rust-IME in your application menu."
fi

# 4.4 Trigger first-time installation tasks
# Use the installed path
/usr/local/bin/rust-ime --install || true

echo -e "\n=========================================="
echo "🎉 Installation Complete!"
echo ""
echo "📋 Next steps:"
echo "   1. Start IME: rust-ime"
echo "   2. Or find 'Rust-IME' in your application menu"
echo ""
if ! groups | grep -q "\binput\b"; then
echo "⚠️  IMPORTANT: You were added to the 'input' group."
echo "   You MUST logout and log back in (or restart) for it to work!"
echo ""
fi
echo "📖 Documentation: ./INSTALL_GUIDE.md or ./INSTALL_GUIDE_ZH.md"
echo "=========================================="
