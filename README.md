# YouTube Player - Minimal Desktop YouTube Player

A lightweight, distraction-free YouTube player built with Rust and Tauri. Watch YouTube videos in a frameless window without comments, suggestions, or other distractions.

## Features

- **Auto-hiding Titlebar**: A standard macOS titlebar whose window buttons fade
  out while the pointer is away, so the video runs edge to edge
- **Resumes Where You Left Off**: Reopens the last video you loaded
- **Always on Top**: Optional, off by default (`⌘T`)
- **URL Auto-conversion**: Automatically converts YouTube URLs to embedded format
- **Keyboard Shortcuts**: 
  - `⌘L` to enter a different video (`⌘N` does the same)
  - `⌘T` to toggle always on top
  - `⌘B` to open the current video in your browser
  - `⌘V` to paste a URL straight from the clipboard
  - `ESC` to close the player
  - `Enter` to load video from URL input
- **Cross-platform**: Works on macOS, Linux, and Windows

## Installation

### For Regular Users

Download the latest release for your platform:

- **macOS**: Download `YouTube Player.dmg` or `YouTube Player.app`
- **Windows**: Download `YouTube Player.msi` or `YouTube Player.exe`
- **Linux**: Download `YouTube Player.AppImage` or `YouTube Player.deb`

### Building from Source

#### Prerequisites

1. Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Install system dependencies:

**macOS**:
```bash
# Xcode Command Line Tools (if not installed)
xcode-select --install
```

**Linux (Ubuntu/Debian)**:
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

**Windows**:
- Install [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

#### Build Steps

1. Clone the repository:
```bash
git clone https://github.com/yourusername/playere.git
cd playere
```

2. Build the application:
```bash
cargo build --release
```

3. Run the application:
```bash
cargo run --release
```

## Usage

### Running with Command Line

```bash
# Run with a YouTube URL
./playere "https://www.youtube.com/watch?v=dQw4w9WgXcQ"

# Run without URL (paste in the app)
./playere
```

### Using the Application

1. **Launch**: Double-click the app icon or run from terminal
2. **Load Video**: 
   - Paste a YouTube URL in the input field
   - Press Enter or click "Load Video"
3. **Move Window**: Drag the titlebar, as with any macOS window
4. **Close**: Click the red window button (fades in on hover) or press ESC

The window buttons are hidden while the pointer is outside the window. Move the
mouse over the player to bring them back.

### Supported URL Formats

The player accepts various YouTube URL formats:
- `https://www.youtube.com/watch?v=VIDEO_ID`
- `https://youtu.be/VIDEO_ID`
- `https://www.youtube.com/embed/VIDEO_ID`
- `https://www.youtube.com/v/VIDEO_ID`

## Building Release Packages

### macOS (.app and .dmg)

```bash
# Build the app
cargo tauri build

# The .app will be in:
# target/release/bundle/macos/YouTube Player.app

# The .dmg will be in:
# target/release/bundle/dmg/YouTube Player.dmg
```

### Windows (.msi and .exe)

```bash
# Build the app
cargo tauri build

# The .msi installer will be in:
# target/release/bundle/msi/YouTube Player.msi

# The .exe will be in:
# target/release/YouTube Player.exe
```

### Linux (.deb, .AppImage)

```bash
# Build the app
cargo tauri build

# The .deb package will be in:
# target/release/bundle/deb/youtube-player_VERSION_amd64.deb

# The AppImage will be in:
# target/release/bundle/appimage/youtube-player_VERSION_amd64.AppImage
```

## Development

### Project Structure

```
playere/
├── src/
│   └── main.rs          # Rust backend code
├── dist/
│   └── index.html       # Frontend HTML/CSS/JS
├── icons/
│   ├── generate-icon.sh # Redraws the icon and its macOS containers
│   ├── icon.png         # Master artwork (1024x1024)
│   ├── AppIcon.icns     # Icon container the .app bundle ships
│   └── Assets.car       # Compiled catalog; pairs with CFBundleIconName
├── tauri.conf.json      # Tauri configuration
├── Cargo.toml           # Rust dependencies
└── build.rs             # Build script
```

### Key Technologies

- **Rust**: Backend logic and window management
- **Tauri**: Framework for building desktop apps
- **HTML/CSS/JavaScript**: Frontend interface
- **YouTube Embed API**: For video playback

### Customization

You can customize the player by modifying:

1. **Window Size**: Edit `width` and `height` in `tauri.conf.json`
2. **Always on Top**: Press `⌘T` at runtime; change the launch default in the
   `with_always_on_top` call in `src/main.rs`
3. **Styling**: Modify the CSS in the inline HTML in `src/main.rs` (the
   webview is served from there, not from `dist/`)
4. **Embed Parameters**: Edit the URL parameters in `src/main.rs`
5. **App Icon**: Edit the shapes in `icons/generate-icon.sh`, re-run it, then
   rebuild with `./build-release.sh`

The last video you loaded is remembered in
`~/Library/Application Support/YouTube Player/state.json` and reopened on the
next launch. Delete that file to start from the URL entry screen. Passing a URL
on the command line takes precedence over it.

### App Icon

`icons/generate-icon.sh` draws the artwork with ImageMagick and compiles the two
containers the bundle ships. Both are committed, so a normal build needs neither
ImageMagick nor Xcode — only regenerating the icon does.

macOS 26 draws an app that carries only a classic `.icns` inside its own white
rounded plate, which leaves the icon looking nested inside a second squircle.
`Assets.car` plus the `CFBundleIconName` key in the bundle's `Info.plist` opts
into the modern path, where the artwork is drawn edge to edge. Keep the two
together — dropping either one brings the plate back.

## Troubleshooting

### Video Not Playing

1. Check your internet connection
2. Ensure the YouTube URL is valid
3. Some videos may have embedding restrictions

### Window Buttons Not Visible

- They fade out whenever the pointer leaves the window; move the mouse over the
  player to bring them back

### App Crashes on Startup

- **FIXED**: The issue was with Tauri's icon processing system causing memory alignment errors on some macOS setups
- Solution: Disabled complex bundle configurations and used minimal bundle settings
- Current version uses a simplified bundle configuration that avoids the problematic icon processing
- App now runs reliably without crashes

### Build Errors

1. Ensure all prerequisites are installed
2. Update Rust: `rustup update`
3. Clean build: `cargo clean && cargo build`

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Uses YouTube's embedded player