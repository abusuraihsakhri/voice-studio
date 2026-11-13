# Voice Studio Native — development preview

Native Rust/Win32 Windows desktop. Uses standard controls and GDI; no Electron, Chromium, WebView, WGPU, OpenGL or Vulkan renderer is part of this application. Windows desktop compositing and native system dialogs remain OS responsibilities.

This is an implementation checkpoint, **not yet a replacement for the full voice studio**. A direct native CPU synthesis profiling path is implemented; GUI generation and voice-cloning workflows are not enabled yet. Actual inference evidence is tracked in `docs/native-migration.md`. Do not uninstall the existing application.

Implemented:

- Native window, DPI-scaled controls, keyboard mnemonics and editable text.
- Independent live RAM, system commit and process-memory monitoring without Torch/CUDA initialization.
- Conservative budget checks and a Windows Job worker runner: suspended launch, assignment before resume, private-commit cap, cancellation, timeout, kill-on-close and confirmed subtree drain.
- Bounded, proxy-free authenticated loopback discovery of existing `/health`, `/engines`, `/profiles`; redirects are refused. Discovery does not load a model and does not contain an externally started backend.
- Separate, atomic, bounded local draft storage; incompatible drafts are preserved rather than overwritten. One window per native data directory.
- A process-wide heavy-stage lease prevents overlapping admitted workers. Unconfirmed cleanup blocks replacement work until restart. Rejected admission releases its lease without launching a process.
- "Check Python" selects an existing interpreter for a bounded, CPU-only module-discovery probe. It does not import Torch, install packages or certify backend/model execution. The new action has command-line/unit coverage; its UI has not been exercised in this checkpoint at the user's request.
- Native file selection and PCM WAV preview, capped at 32 MiB with an owned playback buffer. Stop releases that buffer. Larger/compressed media require the future streaming player.
- Pinned static CPU runtime build/preflight, streaming model integrity checks, explicit resumable developer model fetcher, inherited-handle whitelisting, CPU-limited workers and a fixed native synthesis profiling fixture. Synthetic WAVs receive explicit RIFF metadata; this is not an AudioSeal watermark.

Not implemented: GUI AI generation/cloning, voice library editing, original project import, batch/narration, ASR, dubbing, in-app model installation, engine certification and installer/update integration. Optional Python adapters still need managed setup/launch. The imported Windows GGUF executable is a zero-byte placeholder; the new runtime is built from pinned source and staged separately.

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
& .\native\desktop\target\release\voice-studio-native.exe --runtime-check 'C:\path\to\python.exe' | Out-String
& .\native\desktop\target\release\voice-studio-native.exe --admitted-probe | Out-String
```

The allocation probe asks Windows to commit 16 MiB then 512 MiB inside a 128 MiB Job. The second request must fail; it is not a system-exhaustion stress test. Windows' returned peak Job accounting is recorded as-is and may include a rejected allocation attempt; do not interpret it as physical RAM consumption. No GPU/model is loaded by this probe.

Native drafts are stored at `%LOCALAPPDATA%\VoiceStudioNative\draft.json`. `VOICE_STUDIO_NATIVE_DATA_DIR` can select another absolute, isolated directory. Nothing is written into the old application's project/voice storage. `VOICE_STUDIO_BACKEND_TOKEN`, when explicitly supplied, is sent only as a Bearer header to the selected `127.0.0.1` port and is never stored in the draft or diagnostics.

The current resource reserve is a tuning hypothesis, not a low-end hardware certification. CPU inference can also exhaust RAM. Private-commit limits are not GPU-memory caps. See root `BUILD_SPEC.md` and `docs/native-migration.md` for the remaining implementation and acceptance gates.

## Native CPU engine development

Build from the pinned C++/GGML source using installed CMake and MSVC C++ tools:

```powershell
./scripts/build-native-engine.ps1
```

Configuration/compilation run one compiler task under a 768 MiB Job budget. If an exact verified build already exists, `-PackageExisting` stages and preflights it without compiling again. `scripts/build-native.ps1` includes that verified runtime and its license notices when present; it never bundles model weights or calls the preview inference-ready merely because the runtime exists.

The explicit developer model fetcher uses the pinned catalog and a rate limit:

```powershell
python scripts/download-native-models.py --destination 'N:\absolute\local\model-folder'
& ./native/desktop/target/release/voice-studio-native.exe --model-check 'N:\absolute\local\model-folder' | Out-String
```

The source model pair totals 659,959,328 bytes. Completed files are verified and reused; unidentified/corrupt files are preserved, never silently overwritten. This helper is for development; native inference itself does not require Python.

The engineering-only fixture accepts a verified runtime directory, verified model directory and **new** WAV path:

```powershell
& ./native/desktop/target/release/voice-studio-native.exe --profile-native-cpu 'N:\runtime-folder' 'N:\model-folder' 'N:\new-test.wav' | Out-String
```

It synthesizes a fixed public phrase and records allocation accounting, audio duration/peak, seed, model/runtime identities and hashes. A successful WAV is not proof of content accuracy, voice-cloning quality or general memory capacity. Model checksums do not grant or replace model license rights. The GUI and general job workflow still need integration and validation.
