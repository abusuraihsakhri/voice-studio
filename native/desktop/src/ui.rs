use crate::resource::{self, GIB, ResourcePolicy};
use std::{
    cell::RefCell,
    ptr::{null, null_mut},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Controls::{Dialogs::*, EM_SETLIMITTEXT},
        HiDpi::*,
        WindowsAndMessaging::*,
    },
};

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

struct View {
    status: HWND,
    details: HWND,
    file: HWND,
    text: HWND,
    backend_status: HWND,
    backend_port: HWND,
    pending: Option<Receiver<String>>,
    font: HFONT,
    saved: crate::state::Draft,
    save_allowed: bool,
    edited: Option<Instant>,
    preview_audio: Option<Vec<u8>>,
}
thread_local! { static VIEW: RefCell<Option<View>> = const { RefCell::new(None) }; }

unsafe fn control(parent: HWND, class: &str, label: &str, id: usize, style: u32) -> HWND {
    unsafe {
        let window = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            100,
            30,
            parent,
            id as HMENU,
            GetModuleHandleW(null()),
            null(),
        );
        SendMessageW(
            window,
            WM_SETFONT,
            GetStockObject(DEFAULT_GUI_FONT) as usize,
            1,
        );
        window
    }
}

fn refresh() {
    VIEW.with(|view| {
        if let Some(v) = view.borrow_mut().as_mut() {
            if let Some(receiver) = &v.pending {
                match receiver.try_recv() {
                    Ok(status) => {
                        unsafe { SetWindowTextW(v.backend_status, wide(&status).as_ptr()); }
                        v.pending = None;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        unsafe { SetWindowTextW(v.backend_status, wide("Backend connection interrupted. Retry when ready.").as_ptr()); }
                        v.pending = None;
                    }
                    _ => {}
                }
            }
            let text = match resource::snapshot() {
                Ok(m) => {
                    let p = ResourcePolicy::conservative(&m);
                    format!("Available RAM: {:.2} / {:.2} GiB     System commit headroom: {:.2} GiB\r\nNative UI: {:.1} MiB working set / {:.1} MiB private commit\r\nReserved RAM: {:.2} GiB     Worker commit ceiling: {:.2} GiB\r\nRendering: native Windows controls / GDI. No application GPU renderer.\r\nHeavy-stage limit: 1. Unknown engine memory envelopes are refused.", m.available_ram as f64/GIB as f64, m.total_ram as f64/GIB as f64, m.available_commit as f64/GIB as f64, m.process_working_set as f64/1048576., m.process_private_commit as f64/1048576., p.ram_reserve as f64/GIB as f64, p.worker_commit_limit as f64/GIB as f64)
                }, Err(e) => format!("Resource monitoring unavailable: {e}. Inference blocked.")
            };
            unsafe { SetWindowTextW(v.details, wide(&text).as_ptr()); }
        }
    });
}

unsafe fn text_of(window: HWND, max: usize) -> String {
    unsafe {
        let mut buffer = vec![0u16; max + 1];
        let length =
            GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buffer[..length])
    }
}

fn save_draft(force: bool) -> Result<(), String> {
    VIEW.with(|view| {let mut binding=view.borrow_mut();let Some(v)=binding.as_mut() else {return Ok(())};
        if !force && v.edited.is_none_or(|time|time.elapsed()<Duration::from_secs(3)){return Ok(())}
        let draft=unsafe{crate::state::Draft{text:text_of(v.text,16000),audio:text_of(v.file,32767)}};
        if draft==v.saved {v.edited=None;return Ok(())}
        if !v.save_allowed{return Err("The existing native draft could not be read. It has been preserved; automatic overwrite is disabled.".into())}
        let result=crate::state::draft_path().and_then(|path|crate::state::save(&path,&draft));
        match &result {Ok(())=>{v.saved=draft;v.edited=None;unsafe{SetWindowTextW(v.status,wide("Draft saved locally. AI engine integration is still in progress.").as_ptr());}},Err(error)=>{v.edited=Some(Instant::now());unsafe{SetWindowTextW(v.status,wide(error).as_ptr());}}}
        result
    })
}

unsafe fn layout(hwnd: HWND) {
    unsafe {
        let mut area: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut area);
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let width = (area.right * 96 / dpi).max(640);
        let height = (area.bottom * 96 / dpi).max(600);
        for (id, x, y, w, h) in [
            (1, 24, 18, width - 48, 30),
            (2, 24, 55, width - 48, 25),
            (3, 24, 94, width - 48, 122),
            (11, 24, 230, 130, 28),
            (12, 160, 228, 70, 28),
            (13, 246, 226, 170, 32),
            (18, 432, 226, 184, 32),
            (14, 24, 266, width - 48, 48),
            (4, 24, 330, width - 48, 24),
            (5, 24, 360, width - 48, (height - 553).max(40)),
            (16, 24, height - 174, width - 48, 24),
            (6, 24, height - 142, width - 178, 28),
            (7, width - 144, height - 142, 120, 28),
            (8, 24, height - 100, (width - 84) / 4, 32),
            (9, 36 + (width - 84) / 4, height - 100, (width - 84) / 4, 32),
            (
                15,
                48 + 2 * ((width - 84) / 4),
                height - 100,
                (width - 84) / 4,
                32,
            ),
            (
                17,
                60 + 3 * ((width - 84) / 4),
                height - 100,
                (width - 84) / 4,
                32,
            ),
            (10, 24, height - 52, width - 48, 28),
        ] {
            MoveWindow(
                GetDlgItem(hwnd, id),
                x * dpi / 96,
                y * dpi / 96,
                w * dpi / 96,
                h * dpi / 96,
                1,
            );
        }
    }
}

unsafe fn update_font(hwnd: HWND) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96) as i32;
        let font = CreateFontW(
            -16 * dpi / 96,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide("Segoe UI").as_ptr(),
        );
        if font.is_null() {
            return;
        }
        for id in 1..=18 {
            SendMessageW(GetDlgItem(hwnd, id), WM_SETFONT, font as usize, 1);
        }
        VIEW.with(|view| {
            if let Some(v) = view.borrow_mut().as_mut() {
                let old = v.font;
                v.font = font;
                if !old.is_null() {
                    DeleteObject(old);
                }
            }
        });
    }
}

unsafe fn connect_backend() {
    unsafe {
        VIEW.with(|view| {
            if let Some(v) = view.borrow_mut().as_mut() {
                if v.pending.is_some() {
                    return;
                }
                let mut buffer = [0u16; 16];
                let length =
                    GetWindowTextW(v.backend_port, buffer.as_mut_ptr(), buffer.len() as i32);
                let Ok(port) = String::from_utf16_lossy(&buffer[..length as usize]).parse::<u16>()
                else {
                    SetWindowTextW(
                        v.backend_status,
                        wide("Enter a port between 1 and 65535.").as_ptr(),
                    );
                    return;
                };
                let client = match crate::backend::BackendClient::new(
                    port,
                    std::env::var("VOICE_STUDIO_BACKEND_TOKEN").ok(),
                ) {
                    Ok(client) => client,
                    Err(error) => {
                        SetWindowTextW(v.backend_status, wide(&error).as_ptr());
                        return;
                    }
                };
                let (sender, receiver) = mpsc::sync_channel(1);
                v.pending = Some(receiver);
                SetWindowTextW(
                    v.backend_status,
                    wide("Checking local backend readiness and inventories…").as_ptr(),
                );
                std::thread::spawn(move || {
                    let result = client.inventory().unwrap_or_else(|e| e);
                    let _ = sender.send(result);
                });
            }
        });
    }
}

unsafe fn choose_audio(hwnd: HWND) {
    unsafe {
        let mut buffer = vec![0u16; 32768];
        let filter = wide("Wave audio (*.wav)\0*.wav\0All files\0*.*\0");
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
        dialog.hwndOwner = hwnd;
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut dialog) != 0 {
            VIEW.with(|view| {
                if let Some(v) = view.borrow().as_ref() {
                    SetWindowTextW(v.file, buffer.as_ptr());
                }
            });
            VIEW.with(|view| {
                if let Some(v) = view.borrow_mut().as_mut() {
                    v.edited = Some(Instant::now());
                }
            });
        }
    }
}

unsafe fn check_python(hwnd: HWND) {
    unsafe {
        if VIEW.with(|v| {
            v.borrow()
                .as_ref()
                .is_some_and(|view| view.pending.is_some())
        }) {
            return;
        }
        let mut buffer = vec![0u16; 32768];
        let filter =
            wide("Python interpreter (python.exe)\0python.exe\0Executable files (*.exe)\0*.exe\0");
        let title = wide("Select an existing Python interpreter — no packages will be installed");
        let mut dialog: OPENFILENAMEW = std::mem::zeroed();
        dialog.lStructSize = std::mem::size_of_val(&dialog) as u32;
        dialog.hwndOwner = hwnd;
        dialog.lpstrFile = buffer.as_mut_ptr();
        dialog.nMaxFile = buffer.len() as u32;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrTitle = title.as_ptr();
        dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if GetOpenFileNameW(&mut dialog) == 0 {
            return;
        }
        let length = buffer
            .iter()
            .position(|ch| *ch == 0)
            .unwrap_or(buffer.len());
        use std::os::windows::ffi::OsStringExt;
        let python = std::path::PathBuf::from(std::ffi::OsString::from_wide(&buffer[..length]));
        let (sender, receiver) = mpsc::sync_channel(1);
        VIEW.with(|v|{if let Some(view)=v.borrow_mut().as_mut(){view.pending=Some(receiver);SetWindowTextW(view.backend_status,wide("Checking selected Python in an isolated CPU-only worker; no models or packages will be loaded.").as_ptr());}});
        std::thread::spawn(move || {
            let message = match crate::runtime::check(&python) {
                Ok(report) => {
                    let version = report["python_version"].as_str().unwrap_or("unknown");
                    if report["ready"] == true {
                        format!(
                            "Python {version}: module preflight passed. Backend startup and model execution still need verification."
                        )
                    } else if report["python_version_supported"] != true {
                        format!(
                            "Python {version} is unsupported. Select Python 3.11 or newer; the existing interpreter was not changed."
                        )
                    } else {
                        let missing = report["missing_modules"]
                            .as_array()
                            .map(|modules| {
                                modules
                                    .iter()
                                    .filter_map(|m| m.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            })
                            .unwrap_or_default();
                        format!("Python {version}: missing {missing}. No packages were installed.")
                    }
                }
                Err(error) => error,
            };
            let _ = sender.send(message);
        });
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_CREATE => {
                control(hwnd, "STATIC", "Voice Studio — native Rust desktop", 1, 0);
                control(
                    hwnd,
                    "STATIC",
                    "Local voice tools · software-rendered interface · conservative resources",
                    2,
                    0,
                );
                let details = control(hwnd, "STATIC", "Reading system resources…", 3, 0);
                control(hwnd, "STATIC", "&Local backend port", 11, 0);
                let backend_port = control(
                    hwnd,
                    "EDIT",
                    "3900",
                    12,
                    WS_TABSTOP | WS_BORDER | ES_NUMBER as u32,
                );
                SendMessageW(backend_port, EM_SETLIMITTEXT, 5, 0);
                control(hwnd, "BUTTON", "&Connect backend", 13, WS_TABSTOP);
                control(hwnd, "BUTTON", "Check P&ython…", 18, WS_TABSTOP);
                let backend_status = control(
                    hwnd,
                    "STATIC",
                    "Not connected. Existing backends are discovered only; no model is loaded.",
                    14,
                    0,
                );
                control(hwnd, "STATIC", "&Text to speak", 4, 0);
                let text = control(
                    hwnd,
                    "EDIT",
                    "",
                    5,
                    WS_TABSTOP
                        | WS_BORDER
                        | WS_VSCROLL
                        | ES_MULTILINE as u32
                        | ES_AUTOVSCROLL as u32
                        | ES_WANTRETURN as u32,
                );
                SendMessageW(text, EM_SETLIMITTEXT, 16000, 0);
                control(hwnd, "STATIC", "Selected &audio file", 16, 0);
                let file = control(
                    hwnd,
                    "EDIT",
                    "",
                    6,
                    WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32 | ES_READONLY as u32,
                );
                control(hwnd, "BUTTON", "&Browse audio…", 7, WS_TABSTOP);
                control(hwnd, "BUTTON", "&Refresh", 8, WS_TABSTOP);
                control(hwnd, "BUTTON", "&Play WAV", 9, WS_TABSTOP);
                control(hwnd, "BUTTON", "&Save draft", 15, WS_TABSTOP);
                control(hwnd, "BUTTON", "Stop a&udio", 17, WS_TABSTOP);
                let status = control(
                    hwnd,
                    "STATIC",
                    "Native shell ready. Engine integration is not implemented in this milestone.",
                    10,
                    0,
                );
                let loaded = crate::state::draft_path().and_then(|p| crate::state::load(&p));
                let save_allowed = loaded.is_ok();
                if let Err(error) = &loaded {
                    SetWindowTextW(status, wide(error).as_ptr());
                }
                let saved = loaded.unwrap_or_default();
                SetWindowTextW(text, wide(&saved.text).as_ptr());
                SetWindowTextW(file, wide(&saved.audio).as_ptr());
                VIEW.with(|view| {
                    *view.borrow_mut() = Some(View {
                        status,
                        details,
                        file,
                        text,
                        backend_port,
                        backend_status,
                        pending: None,
                        font: null_mut(),
                        saved,
                        save_allowed,
                        edited: None,
                        preview_audio: None,
                    })
                });
                update_font(hwnd);
                SetTimer(hwnd, 1, 1000, None);
                refresh();
                layout(hwnd);
                0
            }
            WM_CTLCOLORSTATIC => {
                SetBkMode(wparam as HDC, TRANSPARENT as i32);
                GetSysColorBrush(COLOR_WINDOW) as LRESULT
            }
            WM_SIZE => {
                layout(hwnd);
                0
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lparam as *mut MINMAXINFO);
                let dpi = GetDpiForWindow(hwnd).max(96) as i32;
                info.ptMinTrackSize.x = 680 * dpi / 96;
                info.ptMinTrackSize.y = 680 * dpi / 96;
                0
            }
            WM_DPICHANGED => {
                let rect = &*(lparam as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                update_font(hwnd);
                layout(hwnd);
                0
            }
            WM_TIMER => {
                refresh();
                let _ = save_draft(false);
                0
            }
            WM_COMMAND => {
                if wparam & 0xffff == 5 && (wparam >> 16) & 0xffff == EN_CHANGE as usize {
                    VIEW.with(|view| {
                        if let Some(v) = view.borrow_mut().as_mut() {
                            v.edited = Some(Instant::now());
                        }
                    });
                    return 0;
                }
                match wparam & 0xffff {
                    18 => check_python(hwnd),
                    17 => VIEW.with(|view| {
                        if let Some(v) = view.borrow_mut().as_mut() {
                            windows_sys::Win32::Media::Audio::PlaySoundW(null(), null_mut(), 0);
                            v.preview_audio = None;
                            SetWindowTextW(
                                v.status,
                                wide("Playback stopped; preview buffer released.").as_ptr(),
                            );
                        }
                    }),
                    15 => {
                        let _ = save_draft(true);
                    }
                    13 => connect_backend(),
                    7 => choose_audio(hwnd),
                    8 => refresh(),
                    9 => VIEW.with(|view| {
                        if let Some(v) = view.borrow_mut().as_mut() {
                            let mut path = vec![0u16; 32768];
                            GetWindowTextW(v.file, path.as_mut_ptr(), path.len() as i32);
                            let length = path.iter().position(|ch| *ch == 0).unwrap_or(path.len());
                            let selected =
                                std::path::PathBuf::from(String::from_utf16_lossy(&path[..length]));
                            let bytes = match crate::audio::load_preview(&selected) {
                                Ok(bytes) => bytes,
                                Err(error) => {
                                    SetWindowTextW(v.status, wide(&error).as_ptr());
                                    return;
                                }
                            };
                            windows_sys::Win32::Media::Audio::PlaySoundW(null(), null_mut(), 0);
                            v.preview_audio = Some(bytes);
                            let result = windows_sys::Win32::Media::Audio::PlaySoundW(
                                v.preview_audio.as_ref().unwrap().as_ptr() as _,
                                null_mut(),
                                windows_sys::Win32::Media::Audio::SND_MEMORY
                                    | windows_sys::Win32::Media::Audio::SND_ASYNC
                                    | windows_sys::Win32::Media::Audio::SND_NODEFAULT,
                            );
                            SetWindowTextW(
                                v.status,
                                wide(if result != 0 {
                                    "Playing selected WAV."
                                } else {
                                    "Select a valid WAV file to play."
                                })
                                .as_ptr(),
                            );
                        }
                    }),
                    _ => {}
                }
                0
            }
            WM_CLOSE => {
                if let Err(error) = save_draft(true)
                    && MessageBoxW(
                        hwnd,
                        wide(&format!(
                            "{error}\n\nClose without saving the current draft?"
                        ))
                        .as_ptr(),
                        wide("Draft was not saved").as_ptr(),
                        MB_OKCANCEL | MB_ICONWARNING,
                    ) != IDOK
                {
                    return 0;
                }
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                windows_sys::Win32::Media::Audio::PlaySoundW(null(), null_mut(), 0);
                VIEW.with(|v| {
                    if let Some(view) = v.borrow_mut().take()
                        && !view.font.is_null()
                    {
                        DeleteObject(view.font);
                    }
                });
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

pub fn run() -> Result<(), String> {
    unsafe {
        use std::hash::{Hash, Hasher};
        use std::os::windows::io::{FromRawHandle, OwnedHandle};
        let mut identity = std::collections::hash_map::DefaultHasher::new();
        crate::state::draft_path()?
            .to_string_lossy()
            .to_lowercase()
            .hash(&mut identity);
        let mutex = windows_sys::Win32::System::Threading::CreateMutexW(
            null(),
            0,
            wide(&format!("Local\\VoiceStudioNative-{:x}", identity.finish())).as_ptr(),
        );
        if mutex.is_null() {
            return Err("Cannot establish single-instance draft ownership.".into());
        }
        let existed = GetLastError() == ERROR_ALREADY_EXISTS;
        let _instance_guard = OwnedHandle::from_raw_handle(mutex);
        if existed {
            let window = FindWindowW(wide("VoiceStudioNativeWindow").as_ptr(), null());
            if !window.is_null() {
                ShowWindow(window, SW_RESTORE);
                SetForegroundWindow(window);
            }
            return Ok(());
        }
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let class = wide("VoiceStudioNativeWindow");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(window_proc);
        wc.hInstance = instance;
        wc.lpszClassName = class.as_ptr();
        wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        wc.hbrBackground = (COLOR_WINDOW + 1) as HBRUSH;
        if RegisterClassW(&wc) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            wide("Voice Studio Native").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            920,
            800,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result == -1 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if msg.message == WM_KEYDOWN
                && msg.wParam == b'A' as usize
                && windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL as i32,
                ) < 0
            {
                let mut class_name = [0u16; 32];
                let length =
                    GetClassNameW(msg.hwnd, class_name.as_mut_ptr(), class_name.len() as i32);
                if String::from_utf16_lossy(&class_name[..length.max(0) as usize])
                    .eq_ignore_ascii_case("edit")
                {
                    SendMessageW(msg.hwnd, windows_sys::Win32::UI::Controls::EM_SETSEL, 0, -1);
                    continue;
                }
            }
            if IsDialogMessageW(hwnd, &msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    }
}
