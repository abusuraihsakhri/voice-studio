# Voice Studio native Rust fork

Repository: https://github.com/abusuraihsakhri/voice-studio

Goal: replace the Electron desktop with a native Rust UI using software rendering, preserving useful inference engines/backend behavior and adding conservative resource protection for modest PCs.

Current state: supplied VoiceStudio 0.5.8 source imported as a baseline. `native/desktop/` now builds a native Win32/GDI Rust desktop preview with independent telemetry, bounded native WAV playback, local draft persistence, read-only loopback backend discovery and a Windows Job worker primitive. Implementation and remaining gates are in BUILD_SPEC.md and docs/native-migration.md. AI generation is not integrated and low-resource hardware is not certified.

Desktop migration direction in BUILD_SPEC.md overrides inherited upstream Electron-only rules. Electron remains reference source only. Existing licensing and attribution remain in force.

Workflow: commit and push coherent checkpoints; implementation on a development branch; preserve source-baseline-0.5.8. Actions initially disabled because inherited workflows target upstream Electron/mobile/releases; introduce appropriate native CI before enabling.

Completed checkpoint: rendered native release window; draft save/reopen; local discovery fixture; PCM playback/stop; keyboard navigation; nine Rust unit tests; strict Clippy; allocation-limit, cancellation and timeout probes with subtree drain. The UI was observed around 19–22 MiB working set before opening the OS file dialog and around 68 MiB after it. These are workstation snapshots without AI inference.

Next milestone: managed backend/runtime selection and authenticated owned launch, shared resource admission, contained engine integration and a real voice-cloning/TTS output. Portable preview packaging is for development, not a completed application release.
