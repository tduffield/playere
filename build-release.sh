#!/bin/bash

# Build script for YouTube Player
# Creates release packages for all platforms

echo "🚀 Building YouTube Player Release..."

# Ensure we're in the project directory
cd "$(dirname "$0")"

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cargo clean

# Build for current platform
echo "🔨 Building release version..."
cargo build --release

# Get the current platform and create simple app bundle
if [[ "$OSTYPE" == "darwin"* ]]; then
    PLATFORM="macOS"
    echo "✅ Built for macOS"
    
    # Create app bundle structure
    APP_NAME="YouTube Player.app"
    APP_DIR="target/release/bundle/$APP_NAME"
    mkdir -p "$APP_DIR/Contents/MacOS"
    mkdir -p "$APP_DIR/Contents/Resources"
    
    # Copy binary
    cp target/release/playere "$APP_DIR/Contents/MacOS/YouTube Player"
    
    # Copy icon. Assets.car pairs with CFBundleIconName below; without it macOS 26
    # draws the .icns inside its own white plate. Both are named AppIcon.
    if [ -f "icons/AppIcon.icns" ]; then
        cp icons/AppIcon.icns "$APP_DIR/Contents/Resources/AppIcon.icns"
        [ -f "icons/Assets.car" ] && cp icons/Assets.car "$APP_DIR/Contents/Resources/Assets.car"
    else
        echo "⚠️  icons/AppIcon.icns missing — run ./icons/generate-icon.sh"
    fi
    
    # Create Info.plist
    cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>YouTube Player</string>
    <key>CFBundleIdentifier</key>
    <string>com.playere.app</string>
    <key>CFBundleName</key>
    <string>YouTube Player</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIconName</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF
    
    # Refresh the icon cache so Finder picks up the new icon immediately
    touch "$APP_DIR"

    echo "📦 App bundle created at: $APP_DIR"
    echo "  - Binary: target/release/playere"
    
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    PLATFORM="Linux"
    echo "✅ Built for Linux"
    echo "📦 Binary created at: target/release/playere"
    echo ""
    echo "To install system-wide:"
    echo "  sudo cp target/release/playere /usr/local/bin/"
    
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || "$OSTYPE" == "win32" ]]; then
    PLATFORM="Windows"
    echo "✅ Built for Windows"
    echo "📦 Executable created at: target/release/playere.exe"
fi

echo ""
echo "🎉 Build complete!"