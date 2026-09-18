use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wry::{
    application::{
        accelerator::{Accelerator, SysMods},
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        keyboard::KeyCode,
        menu::{MenuBar, MenuId, MenuItem, MenuItemAttributes},
        window::{Window, WindowBuilder},
    },
    webview::WebViewBuilder,
};

#[cfg(target_os = "macos")]
use wry::application::platform::macos::WindowBuilderExtMacOS;

/// Fades the close/minimise/zoom buttons in and out. macOS has no built-in
/// auto-hiding titlebar outside fullscreen, so the webview reports hover over
/// IPC and we drive the buttons' alpha from here.
#[cfg(target_os = "macos")]
fn set_titlebar_buttons_alpha(window: &Window, alpha: f64) {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use wry::application::platform::macos::WindowExtMacOS;

    let ns_window = window.ns_window() as *mut Object;
    if ns_window.is_null() {
        return;
    }
    // NSWindowCloseButton = 0, NSWindowMiniaturizeButton = 1, NSWindowZoomButton = 2
    for index in 0u64..3 {
        unsafe {
            let button: *mut Object = msg_send![ns_window, standardWindowButton: index];
            if !button.is_null() {
                let _: () = msg_send![button, setAlphaValue: alpha];
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_titlebar_buttons_alpha(_window: &Window, _alpha: f64) {}

/// True while the pointer is within the window's frame. Polled rather than
/// driven from the webview: the YouTube iframe swallows mouse events, so the
/// page sees the cursor leave whenever it moves over the video.
#[cfg(target_os = "macos")]
fn cursor_is_over_window(window: &Window) -> bool {
    use cocoa::appkit::NSEvent;
    use cocoa::base::nil;
    use cocoa::foundation::NSRect;
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use wry::application::platform::macos::WindowExtMacOS;

    let ns_window = window.ns_window() as *mut Object;
    if ns_window.is_null() {
        return false;
    }
    unsafe {
        // Both are screen coordinates with a bottom-left origin, so they compare
        // directly without converting between coordinate spaces.
        let frame: NSRect = msg_send![ns_window, frame];
        let cursor = NSEvent::mouseLocation(nil);
        cursor.x >= frame.origin.x
            && cursor.x <= frame.origin.x + frame.size.width
            && cursor.y >= frame.origin.y
            && cursor.y <= frame.origin.y + frame.size.height
    }
}

#[cfg(not(target_os = "macos"))]
fn cursor_is_over_window(_window: &Window) -> bool {
    true
}

/// The clipboard's text, if it holds any.
#[cfg(target_os = "macos")]
fn clipboard_text() -> Option<String> {
    use cocoa::base::nil;
    use cocoa::foundation::NSString;
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let pasteboard: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        if pasteboard.is_null() {
            return None;
        }
        let kind = NSString::alloc(nil).init_str("public.utf8-plain-text");
        let value: *mut Object = msg_send![pasteboard, stringForType: kind];
        if value.is_null() {
            return None;
        }
        let bytes: *const std::os::raw::c_char = msg_send![value, UTF8String];
        let text = std::ffi::CStr::from_ptr(bytes).to_string_lossy().into_owned();
        Some(text).filter(|t| !t.trim().is_empty())
    }
}

#[cfg(not(target_os = "macos"))]
fn clipboard_text() -> Option<String> {
    None
}

/// Where the last-played URL is remembered between launches.
fn state_file() -> Option<PathBuf> {
    let home = env::var_os("HOME")?;
    let dir = if cfg!(target_os = "macos") {
        PathBuf::from(home).join("Library/Application Support/YouTube Player")
    } else {
        PathBuf::from(home).join(".config/youtube-player")
    };
    Some(dir.join("state.json"))
}

fn read_last_url() -> Option<String> {
    let raw = fs::read_to_string(state_file()?).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    value
        .get("last_url")?
        .as_str()
        .filter(|url| !url.is_empty())
        .map(str::to_string)
}

fn write_last_url(url: &str) {
    let Some(path) = state_file() else { return };
    if let Some(dir) = path.parent() {
        if let Err(e) = fs::create_dir_all(dir) {
            eprintln!("Could not create state directory: {}", e);
            return;
        }
    }
    let body = serde_json::json!({ "last_url": url }).to_string();
    if let Err(e) = fs::write(&path, body) {
        eprintln!("Could not save last video: {}", e);
    }
}

fn main() -> wry::Result<()> {
    let args: Vec<String> = env::args().collect();
    let initial_url = if args.len() > 1 {
        args[1].clone()
    } else {
        read_last_url().unwrap_or_default()
    };

    let event_loop = EventLoop::new();
    // The shortcuts live in a real menu rather than in page JavaScript: the
    // YouTube iframe is cross-origin, so once it has focus it swallows every
    // keystroke and the page never sees them. Menu accelerators are handled by
    // the system before the webview, and they are discoverable besides.
    let menu_play_clipboard = MenuId::new("play_clipboard");
    let menu_enter_url = MenuId::new("enter_url");
    let menu_toggle_pin = MenuId::new("toggle_pin");
    let menu_open_browser = MenuId::new("open_browser");

    let mut app_menu = MenuBar::new();
    app_menu.add_native_item(MenuItem::Hide);
    app_menu.add_native_item(MenuItem::HideOthers);
    app_menu.add_native_item(MenuItem::Separator);
    app_menu.add_native_item(MenuItem::Quit);

    let mut video_menu = MenuBar::new();
    video_menu.add_item(
        MenuItemAttributes::new("Play URL from Clipboard")
            .with_id(menu_play_clipboard)
            .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyL)),
    );
    video_menu.add_item(
        MenuItemAttributes::new("Enter URL…")
            .with_id(menu_enter_url)
            .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyN)),
    );
    video_menu.add_native_item(MenuItem::Separator);
    video_menu.add_item(
        MenuItemAttributes::new("Always on Top")
            .with_id(menu_toggle_pin)
            .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyT)),
    );
    video_menu.add_item(
        MenuItemAttributes::new("Open in Browser")
            .with_id(menu_open_browser)
            .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyB)),
    );
    video_menu.add_native_item(MenuItem::Separator);
    video_menu.add_native_item(MenuItem::CloseWindow);

    // Without these the webview gets no clipboard or selection shortcuts at all,
    // so the URL field could not be pasted into.
    let mut edit_menu = MenuBar::new();
    edit_menu.add_native_item(MenuItem::Undo);
    edit_menu.add_native_item(MenuItem::Redo);
    edit_menu.add_native_item(MenuItem::Separator);
    edit_menu.add_native_item(MenuItem::Cut);
    edit_menu.add_native_item(MenuItem::Copy);
    edit_menu.add_native_item(MenuItem::Paste);
    edit_menu.add_native_item(MenuItem::SelectAll);

    let mut window_menu = MenuBar::new();
    window_menu.add_native_item(MenuItem::Minimize);
    window_menu.add_native_item(MenuItem::Zoom);
    window_menu.add_native_item(MenuItem::Separator);
    window_menu.add_native_item(MenuItem::EnterFullScreen);

    let mut menu = MenuBar::new();
    menu.add_submenu("YouTube Player", true, app_menu);
    menu.add_submenu("Video", true, video_menu);
    menu.add_submenu("Edit", true, edit_menu);
    menu.add_submenu("Window", true, window_menu);

    let builder = WindowBuilder::new()
        .with_menu(menu)
        .with_title("YouTube Player")
        .with_always_on_top(false)
        .with_resizable(true)
        .with_inner_size(wry::application::dpi::LogicalSize::new(960, 540))
        .with_min_inner_size(wry::application::dpi::LogicalSize::new(480, 270));

    // A titled window is what gives macOS its rounded corners; the transparent
    // titlebar over a full-size content view keeps the video edge to edge.
    #[cfg(target_os = "macos")]
    let builder = builder
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true);

    let window = builder.build(&event_loop)?;

    // Start hidden; the buttons fade in when the cursor enters the window.
    set_titlebar_buttons_alpha(&window, 0.0);

    let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>YouTube Player</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        
        body {{
            background: #0a0a0a;
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            color: #ffffff;
            overflow: hidden;
            user-select: none;
        }}
        
        .app-container {{
            width: 100vw;
            height: 100vh;
            position: relative;
            display: flex;
            flex-direction: column;
        }}
        
        /* Nothing paints under the native titlebar, but keep clear of the
           traffic lights when they fade in. */
        .pin-toast {{
            position: absolute;
            top: 44px;
            left: 50%;
            transform: translateX(-50%);
            background: rgba(0, 0, 0, 0.8);
            backdrop-filter: blur(10px);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 6px;
            color: rgba(255, 255, 255, 0.9);
            font-size: 12px;
            padding: 6px 12px;
            z-index: 1000;
            opacity: 0;
            pointer-events: none;
            transition: opacity 0.2s ease;
        }}
        
        .pin-toast.show {{
            opacity: 1;
        }}
        
        .main-content {{
            flex: 1;
            position: relative;
            background: #0a0a0a;
        }}
        
        .video-player {{
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            border: none;
            z-index: 1;
        }}
        
        .url-input-container {{
            position: absolute;
            top: 50%;
            left: 50%;
            transform: translate(-50%, -50%);
            background: rgba(0, 0, 0, 0.9);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 12px;
            padding: 32px;
            min-width: 400px;
            text-align: center;
            backdrop-filter: blur(20px);
        }}
        
        .url-input-container h1 {{
            font-size: 24px;
            font-weight: 600;
            margin-bottom: 8px;
            color: #ffffff;
        }}
        
        .url-input-container p {{
            font-size: 14px;
            color: rgba(255, 255, 255, 0.7);
            margin-bottom: 24px;
        }}
        
        .input-group {{
            display: flex;
            flex-direction: column;
            gap: 16px;
        }}
        
        .url-input {{
            width: 100%;
            padding: 12px 16px;
            background: rgba(255, 255, 255, 0.1);
            border: 1px solid rgba(255, 255, 255, 0.2);
            border-radius: 8px;
            color: #ffffff;
            font-size: 14px;
            outline: none;
            transition: all 0.2s ease;
        }}
        
        .url-input:focus {{
            border-color: #3b82f6;
            box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
        }}
        
        .url-input::placeholder {{
            color: rgba(255, 255, 255, 0.5);
        }}
        
        .button-group {{
            display: flex;
            gap: 12px;
        }}
        
        .btn {{
            flex: 1;
            padding: 12px 24px;
            border: none;
            border-radius: 8px;
            font-size: 14px;
            font-weight: 500;
            cursor: pointer;
            transition: all 0.2s ease;
        }}
        
        .btn-primary {{
            background: #3b82f6;
            color: #ffffff;
        }}
        
        .btn-primary:hover {{
            background: #2563eb;
        }}
        
        .btn-secondary {{
            background: rgba(255, 255, 255, 0.1);
            color: #ffffff;
            border: 1px solid rgba(255, 255, 255, 0.2);
        }}
        
        .btn-secondary:hover {{
            background: rgba(255, 255, 255, 0.2);
        }}
        
        .hidden {{
            display: none;
        }}
        
        .loading {{
            opacity: 0.7;
            pointer-events: none;
        }}
        
        .hover-zone {{
            position: absolute;
            bottom: 0;
            left: 0;
            right: 0;
            height: 100px;
            z-index: 2;
            pointer-events: none;
        }}
        
        @media (max-width: 480px) {{
            .url-input-container {{
                min-width: 320px;
                padding: 24px;
            }}
            
            .button-group {{
                flex-direction: column;
            }}
        }}
    </style>
</head>
<body>
    <div class="app-container">
        <div class="pin-toast" id="pin-toast"></div>

        <div class="main-content">
            <iframe id="video-player" class="video-player hidden" 
                    allowfullscreen 
                    allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share; fullscreen"
                    referrerpolicy="strict-origin-when-cross-origin">
            </iframe>
            
            <div id="url-input-container" class="url-input-container">
                <h1>YouTube Player</h1>
                <p>Enter a YouTube video or playlist URL to start watching</p>
                
                <div class="input-group">
                    <input type="text" 
                           id="url-input" 
                           class="url-input" 
                           placeholder="https://www.youtube.com/watch?v=... or playlist"
                           title="Enter YouTube video or playlist URL"
                           autocomplete="off"
                           spellcheck="false">
                    
                    <div class="button-group">
                        <button id="paste-btn" class="btn btn-secondary" onclick="pasteFromClipboard()" title="Paste YouTube URL from clipboard">
                            📋 Paste
                        </button>
                        <button id="load-btn" class="btn btn-primary" onclick="loadVideo()" title="Load the entered video or playlist">
                            Load Video
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </div>
    
    <script>
        const state = {{
            currentVideoId: null,
            currentPlaylistId: null,
            watchingVideo: false,
            isPlaylist: false
        }};
        
        const elements = {{
            urlInput: document.getElementById('url-input'),
            urlContainer: document.getElementById('url-input-container'),
            videoPlayer: document.getElementById('video-player'),
            pasteBtn: document.getElementById('paste-btn'),
            loadBtn: document.getElementById('load-btn'),
            pinToast: document.getElementById('pin-toast')
        }};
        
        function extractVideoId(url) {{
            const patterns = [
                /(?:youtube\.com\/watch\?v=|youtu\.be\/)([a-zA-Z0-9_-]{{11}})/,
                /youtube\.com\/embed\/([a-zA-Z0-9_-]{{11}})/,
                /youtube\.com\/v\/([a-zA-Z0-9_-]{{11}})/
            ];
            
            for (const pattern of patterns) {{
                const match = url.match(pattern);
                if (match && match[1]) {{
                    return match[1];
                }}
            }}
            return null;
        }}
        
        function extractPlaylistId(url) {{
            // More comprehensive playlist detection
            const patterns = [
                /[?&]list=([a-zA-Z0-9_-]+)/,
                /youtube\.com\/playlist\?list=([a-zA-Z0-9_-]+)/,
                /youtu\.be\/[^?]*\?.*list=([a-zA-Z0-9_-]+)/
            ];
            
            for (const pattern of patterns) {{
                const match = url.match(pattern);
                if (match && match[1]) {{
                    // Filter out invalid playlist IDs (like single video watch later, etc.)
                    const playlistId = match[1];
                    if (playlistId.length > 10 && !playlistId.startsWith('WL')) {{
                        return playlistId;
                    }}
                }}
            }}
            return null;
        }}
        
        function parseYouTubeUrl(url) {{
            try {{
                const videoId = extractVideoId(url);
                const playlistId = extractPlaylistId(url);
                
                // Debug logging
                console.log('Parsing URL:', url);
                console.log('Video ID:', videoId);
                console.log('Playlist ID:', playlistId);
                
                return {{
                    videoId,
                    playlistId,
                    isPlaylist: !!playlistId,
                    isVideo: !!videoId
                }};
            }} catch (error) {{
                console.log('Error parsing YouTube URL:', error);
                return {{
                    videoId: null,
                    playlistId: null,
                    isPlaylist: false,
                    isVideo: false
                }};
            }}
        }}
        
        function createEmbedUrl(params) {{
            if (typeof params === 'string') {{
                // Backward compatibility - single video ID
                return `https://www.youtube.com/embed/${{params}}?autoplay=1&controls=1&rel=1&fs=1&modestbranding=1&playsinline=1&enablejsapi=1&origin=${{window.location.origin}}`;
            }}
            
            const {{ videoId, playlistId, isPlaylist }} = params;
            let baseUrl = 'https://www.youtube.com/embed/';
            let queryParams = new URLSearchParams({{
                autoplay: '1',
                controls: '1',
                fs: '1',
                modestbranding: '1',
                playsinline: '1',
                enablejsapi: '1',
                origin: window.location.origin,
                // Enable navigation tracking
                widget_referrer: window.location.origin
            }});
            
            if (isPlaylist && playlistId) {{
                // For playlists, we can optionally start with a specific video
                if (videoId) {{
                    baseUrl += videoId;
                    queryParams.set('list', playlistId);
                }} else {{
                    baseUrl += `videoseries`;
                    queryParams.set('list', playlistId);
                }}
                // Enable related videos within the playlist
                queryParams.set('rel', '1');
            }} else if (videoId) {{
                baseUrl += videoId;
                // Enable related videos for better navigation
                queryParams.set('rel', '1');
            }}
            
            return `${{baseUrl}}?${{queryParams.toString()}}`;
        }}
        
        function showError(message) {{
            elements.urlInput.style.borderColor = '#ef4444';
            elements.urlInput.style.boxShadow = '0 0 0 3px rgba(239, 68, 68, 0.1)';
            elements.urlInput.focus();
            elements.urlInput.select();
            
            setTimeout(() => {{
                elements.urlInput.style.borderColor = 'rgba(255, 255, 255, 0.2)';
                elements.urlInput.style.boxShadow = 'none';
            }}, 2000);
        }}
        
        function loadVideoFromUrl(url, loadingElement, loadingText) {{
            if (!url) {{
                showError('Please enter a YouTube URL');
                return false;
            }}
            
            const urlData = parseYouTubeUrl(url);
            
            if (!urlData.isVideo && !urlData.isPlaylist) {{
                showError('Invalid YouTube URL or playlist');
                return false;
            }}
            
            if (loadingElement) {{
                loadingElement.classList.add('loading');
                if (loadingText) loadingElement.textContent = loadingText;
            }}
            
            if (urlData.isPlaylist) {{
                state.currentPlaylistId = urlData.playlistId;
                state.currentVideoId = urlData.videoId; // might be null
            }} else {{
                state.currentVideoId = urlData.videoId;
                state.currentPlaylistId = null;
            }}
            
            elements.videoPlayer.src = createEmbedUrl(urlData);
            
            // Remembered so the next launch reopens this video.
            if (window.ipc) {{
                window.ipc.postMessage(`save:${{url}}`);
            }}
            
            setTimeout(() => {{
                elements.videoPlayer.classList.remove('hidden');
                elements.urlContainer.classList.add('hidden');
                if (loadingElement) {{
                    loadingElement.classList.remove('loading');
                    if (loadingText) {{
                        loadingElement.textContent = urlData.isPlaylist ? 'Load Playlist' : 'Load Video';
                    }}
                }}
                state.watchingVideo = true;
                state.isPlaylist = urlData.isPlaylist;
                
                // Start monitoring for navigation
                lastVideoId = state.currentVideoId;
                startNavigationMonitoring();
                
                // Set up iframe load event listener for navigation detection
                elements.videoPlayer.onload = () => {{
                    console.log('Iframe loaded, checking for navigation');
                    setTimeout(() => {{
                        try {{
                            const currentUrl = elements.videoPlayer.src;
                            const newVideoId = extractVideoId(currentUrl);
                            if (newVideoId && newVideoId !== lastVideoId) {{
                                console.log('Navigation detected via iframe load:', lastVideoId, '->', newVideoId);
                                handleVideoNavigation(newVideoId);
                            }}
                        }} catch (e) {{
                            console.log('Error checking navigation:', e);
                        }}
                    }}, 1000);
                }};
            }}, 500);
            
            return true;
        }}
        
        function loadVideo() {{
            const url = elements.urlInput.value.trim();
            loadVideoFromUrl(url, elements.loadBtn, 'Loading...');
        }}
        
        let toastTimer;
        function showToast(text, duration = 1400) {{
            elements.pinToast.textContent = text;
            elements.pinToast.classList.add('show');
            clearTimeout(toastTimer);
            toastTimer = setTimeout(() => {{
                elements.pinToast.classList.remove('show');
            }}, duration);
        }}
        
        
        async function pasteFromClipboard() {{
            elements.pasteBtn.classList.add('loading');
            elements.pasteBtn.textContent = 'Pasting...';
            
            try {{
                console.log('Starting paste operation...');
                
                if (!navigator.clipboard) {{
                    throw new Error('Clipboard API not available');
                }}
                
                if (!navigator.clipboard.readText) {{
                    throw new Error('Clipboard readText not supported');
                }}
                
                console.log('Clipboard API available, reading text...');
                const text = await navigator.clipboard.readText();
                console.log('Clipboard text:', text);
                
                if (!text || !text.trim()) {{
                    throw new Error('Clipboard is empty');
                }}
                
                const trimmedText = text.trim();
                console.log('Trimmed text:', trimmedText);
                
                if (!trimmedText.includes('youtube.com') && !trimmedText.includes('youtu.be')) {{
                    throw new Error('No YouTube URL found in clipboard');
                }}
                
                console.log('YouTube URL detected, parsing...');
                const urlData = parseYouTubeUrl(trimmedText);
                console.log('URL data:', urlData);
                
                if (!urlData) {{
                    throw new Error('Failed to parse URL');
                }}
                
                if (!urlData.isVideo && !urlData.isPlaylist) {{
                    throw new Error('URL is not a valid YouTube video or playlist');
                }}
                
                console.log('URL is valid, setting input value...');
                elements.urlInput.value = trimmedText;
                elements.urlInput.focus();
                elements.urlInput.select();
                
                const contentType = urlData.isPlaylist ? 'Playlist' : 'Video';
                elements.pasteBtn.textContent = `✓ ${{contentType}} Pasted`;
                elements.pasteBtn.style.background = '#10b981';
                elements.pasteBtn.style.color = '#000';
                
                console.log('Paste successful!');
                
            }} catch (error) {{
                console.error('Paste failed:', error.message);
                elements.pasteBtn.textContent = 'Paste Failed';
                elements.pasteBtn.style.background = '#ef4444';
                elements.pasteBtn.style.color = '#fff';
                elements.urlInput.focus();
            }}
            
            setTimeout(() => {{
                elements.pasteBtn.classList.remove('loading');
                elements.pasteBtn.textContent = '📋 Paste';
                elements.pasteBtn.style.background = '';
                elements.pasteBtn.style.color = '';
            }}, 2000);
        }}
        
        function closeApp() {{
            if (window.ipc) {{
                window.ipc.postMessage('close');
            }} else {{
                window.close();
            }}
        }}
        
        function showUrlInput() {{
            elements.videoPlayer.classList.add('hidden');
            elements.urlContainer.classList.remove('hidden');
            elements.urlInput.focus();
            elements.urlInput.select();
            state.watchingVideo = false;
            
            // Stop monitoring when not watching
            stopNavigationMonitoring();
        }}
        
        function openInBrowser() {{
            let url = '';
            
            if (state.isPlaylist && state.currentPlaylistId) {{
                url = `https://www.youtube.com/playlist?list=${{state.currentPlaylistId}}`;
                if (state.currentVideoId) {{
                    url = `https://www.youtube.com/watch?v=${{state.currentVideoId}}&list=${{state.currentPlaylistId}}`;
                }}
            }} else if (state.currentVideoId) {{
                url = `https://www.youtube.com/watch?v=${{state.currentVideoId}}`;
            }}
            
            if (url && window.ipc) {{
                window.ipc.postMessage(`browser:${{url}}`);
            }}
        }}
        
        // Event listeners
        elements.urlInput.addEventListener('keydown', (e) => {{
            if (e.key === 'Enter') {{
                e.preventDefault();
                loadVideo();
            }}
        }});
        
        // Only non-modifier keys are handled here. Everything reached with the
        // command key is a menu accelerator, because a focused YouTube iframe
        // never forwards keystrokes to this document.
        document.addEventListener('keydown', (e) => {{
            if (e.key === 'Escape') {{
                closeApp();
            }}
        }});
        
        // Initialize
        window.addEventListener('load', () => {{
            elements.urlInput.focus();
            
            // Load initial URL if provided
            const initialUrl = "{}";
            if (initialUrl && initialUrl.trim()) {{
                elements.urlInput.value = initialUrl.trim();
                setTimeout(() => loadVideo(), 100);
            }}
        }});
        
        // Smart navigation detection system
        let lastVideoId = null;
        let navigationCheckInterval = null;
        let clickInterceptor = null;
        
        function startNavigationMonitoring() {{
            if (navigationCheckInterval) return;
            
            navigationCheckInterval = setInterval(() => {{
                try {{
                    const iframe = elements.videoPlayer;
                    if (!iframe.src) return;
                    
                    // Extract current video ID from iframe src
                    const currentVideoId = extractVideoId(iframe.src);
                    
                    if (currentVideoId && currentVideoId !== lastVideoId) {{
                        console.log('Video navigation detected:', lastVideoId, '->', currentVideoId);
                        
                        // Update our state
                        state.currentVideoId = currentVideoId;
                        lastVideoId = currentVideoId;

                    }}
                }} catch (e) {{
                    // Ignore cross-origin errors
                }}
            }}, 1000);
        }}
        
        function stopNavigationMonitoring() {{
            if (navigationCheckInterval) {{
                clearInterval(navigationCheckInterval);
                navigationCheckInterval = null;
            }}
        }}
        
        
        // Listen for YouTube postMessage events
        window.addEventListener('message', (event) => {{
            if (event.origin === 'https://www.youtube.com') {{
                console.log('YouTube message:', event.data);
                
                // Handle YouTube API events
                if (event.data && typeof event.data === 'string') {{
                    try {{
                        const data = JSON.parse(event.data);
                        if (data.event === 'video-progress' && data.info) {{
                            // Video progress update
                            const videoData = data.info;
                            if (videoData.videoId && videoData.videoId !== state.currentVideoId) {{
                                console.log('Video changed via YouTube navigation');
                                handleVideoNavigation(videoData.videoId);
                            }}
                        }}
                    }} catch (e) {{
                        // Not JSON data, ignore
                    }}
                }}
            }}
        }});
        
        function handleVideoNavigation(newVideoId) {{
            console.log('Handling navigation to video:', newVideoId);
            
            // Prevent infinite loops
            if (newVideoId === state.currentVideoId || newVideoId === lastVideoId) {{
                return;
            }}
            
            // Update our state
            const oldVideoId = state.currentVideoId;
            state.currentVideoId = newVideoId;
            lastVideoId = newVideoId;
            
            // Create new embed URL for the new video
            const newUrl = createEmbedUrl({{
                videoId: newVideoId,
                playlistId: state.currentPlaylistId,
                isPlaylist: state.isPlaylist,
                isVideo: true
            }});
            
            // Temporarily stop monitoring to prevent loop
            stopNavigationMonitoring();
            
            // Update the iframe source
            elements.videoPlayer.src = newUrl;
            
            console.log(`🎥 Video Navigation: ${{oldVideoId}} -> ${{newVideoId}}`);
            
            // Restart monitoring after a delay
            setTimeout(() => {{
                startNavigationMonitoring();
            }}, 2000);
        }}
        
        // Disable context menu
        window.addEventListener('contextmenu', e => e.preventDefault());
    </script>
</body>
</html>
"#, initial_url.replace("\\", ""));

    let webview = WebViewBuilder::new(window)?
        .with_html(html)?
        .with_ipc_handler(move |_window, message| {
            match message.as_str() {
                "close" => {
                    std::process::exit(0);
                }
                msg if msg.starts_with("browser:") => {
                    let url = &msg["browser:".len()..];
                    if let Err(e) = std::process::Command::new("open").arg(url).spawn() {
                        eprintln!("Could not open browser: {}", e);
                    }
                }
                msg if msg.starts_with("save:") => {
                    write_last_url(&msg["save:".len()..]);
                }
                msg if msg.starts_with("navigate:") => {
                    println!("Navigation request: {}", msg);
                }
                _ => {}
            }
        })
        .with_navigation_handler(|_url| {
            // println!("Navigation to: {}", url);
            true // Allow all navigation
        })
        .build()?;

    // macOS has no auto-hiding titlebar outside fullscreen, so the buttons are
    // faded by hand. 150ms is frequent enough to feel immediate without keeping
    // the event loop busy.
    const HOVER_POLL: Duration = Duration::from_millis(150);
    let mut buttons_visible = false;
    let mut pinned = false;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + HOVER_POLL);

        match event {
            Event::NewEvents(StartCause::Init) => println!("YouTube Player started"),
            Event::NewEvents(_) => {
                let window = webview.window();
                let hovered = cursor_is_over_window(window);
                if hovered != buttons_visible {
                    buttons_visible = hovered;
                    set_titlebar_buttons_alpha(window, if hovered { 1.0 } else { 0.0 });
                }
            }
            Event::MenuEvent { menu_id, .. } => {
                let window = webview.window();
                if menu_id == menu_play_clipboard {
                    match clipboard_text() {
                        // loadVideoFromUrl already validates and reports a bad URL.
                        Some(url) => {
                            let script =
                                format!("loadVideoFromUrl({})", serde_json::json!(url.trim()));
                            let _ = webview.evaluate_script(&script);
                        }
                        None => {
                            let _ = webview.evaluate_script(
                                "showToast('Clipboard is empty — copy a YouTube link first')",
                            );
                        }
                    }
                } else if menu_id == menu_enter_url {
                    let _ = webview.evaluate_script("showUrlInput()");
                } else if menu_id == menu_toggle_pin {
                    pinned = !pinned;
                    window.set_always_on_top(pinned);
                    let message = if pinned {
                        "Always on top: on"
                    } else {
                        "Always on top: off"
                    };
                    let _ = webview
                        .evaluate_script(&format!("showToast({})", serde_json::json!(message)));
                } else if menu_id == menu_open_browser {
                    let _ = webview.evaluate_script("openInBrowser()");
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}