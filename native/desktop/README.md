# Voice Studio Native — development preview

Native Rust/Win32 Windows desktop. Uses standard controls and GDI; no Electron, Chromium, WebView, WGPU, OpenGL or Vulkan renderer is part of this application. Windows desktop compositing and native system dialogs remain OS responsibilities.

This is an implementation checkpoint, **not yet a replacement for the full voice studio**. It does not synthesize speech or clone voices yet. Do not uninstall the existing application.

Implemented:

- Native window, DPI-scaled controls, keyboard mnemonics and editable text.
- Independent live RAM, system commit and process-memory monitoring without Torch/CUDA initialization.
- Conservative budget checks and a Windows Job worker runner: suspended launch, assignment before resume, private-commit cap, cancellation, timeout, kill-on-close and confirmed subtree drain.
- Bounded, proxy-free authenticated loopback discovery of existing `/health`, `/engines`, `/profiles`; redirects are refused. Discovery does not load a model and does not contain an externally started backend.
- Separate, atomic, bounded local draft storage; incompatible drafts are preserved rather than overwritten. One window per native data directory.
- Native file selection and PCM WAV preview, capped at 32 MiB with an owned playback buffer. Stop releases that buffer. Larger/compressed media require the future streaming player.

Not implemented: managed Python setup/launch, AI generation, voice library editing, original project import, batch/narration, ASR, dubbing, model installation, engine certification and installer/update integration. The imported Windows GGUF executable is a zero-byte placeholder; it cannot serve as the inference runtime.

## Build and test

Requires Windows x64 and a Rust MSVC toolchain with Visual Studio C++ build tools. Run from the repository root:

```powershell
cargo test --locked --manifest-path native/desktop/Cargo.toml
cargo clippy --locked --manifest-path native/desktop/Cargo.toml --all-targets -- -D warnings
cargo build --release --locked --manifest-path native/desktop/Cargo.toml
& .\native\desktop\target\release\voice-studio-native.exe
```

`scripts/build-native.ps1` performs checks and creates a portable preview ZIP under ignored `dist/`. The unsigned executable is not a production installer.

Diagnostics (pipe GUI-subsystem output to make PowerShell wait):

```powershell
& .\native\desktop\target\release\voice-studio-native.exe --diagnostics | Out-String
& .\native\desktop\target\release\voice-studio-native.exe --containment-test | Out-String
& .\native\desktop\target\release\voice-studio-native.exe --check-budget 1024 1024 | Out-String
```

The allocation probe asks Windows to commit 16 MiB then 512 MiB inside a 128 MiB Job. The second request must fail; it is not a system-exhaustion stress test. Windows' returned peak Job accounting is recorded as-is and may include a rejected allocation attempt; do not interpret it as physical RAM consumption. No GPU/model is loaded by this probe.

Native drafts are stored at `%LOCALAPPDATA%\VoiceStudioNative\draft.json`. `VOICE_STUDIO_NATIVE_DATA_DIR` can select another absolute, isolated directory. Nothing is written into the old application's project/voice storage. `VOICE_STUDIO_BACKEND_TOKEN`, when explicitly supplied, is sent only as a Bearer header to the selected `127.0.0.1` port and is never stored in the draft or diagnostics.

The current resource reserve is a tuning hypothesis, not a low-end hardware certification. CPU inference can also exhaust RAM. Private-commit limits are not GPU-memory caps. See root `BUILD_SPEC.md` and `docs/native-migration.md` for the remaining implementation and acceptance gates.
