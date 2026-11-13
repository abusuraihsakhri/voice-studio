# Voice Studio native Rust fork

Repository: https://github.com/abusuraihsakhri/voice-studio

Goal: replace the Electron desktop with a native Rust UI using software rendering, preserving useful inference engines/backend behavior and adding conservative resource protection for modest PCs.

Current state: supplied VoiceStudio 0.5.8 source imported as a baseline. `native/desktop/` now builds a native Win32/GDI Rust desktop preview with independent telemetry, bounded native WAV playback, local draft persistence, read-only loopback backend discovery and a Windows Job worker primitive. Implementation and remaining gates are in BUILD_SPEC.md and docs/native-migration.md. AI generation is not integrated and low-resource hardware is not certified.

Desktop migration direction in BUILD_SPEC.md overrides inherited upstream Electron-only rules. Electron remains reference source only. Existing licensing and attribution remain in force.

Workflow: commit and push coherent checkpoints; implementation on a development branch; preserve source-baseline-0.5.8. Actions initially disabled because inherited workflows target upstream Electron/mobile/releases; introduce appropriate native CI before enabling.

Completed checkpoint: rendered native release window; draft save/reopen; local discovery fixture; PCM playback/stop; keyboard navigation; nine Rust unit tests; strict Clippy; allocation-limit, cancellation and timeout probes with subtree drain. The UI was observed around 19–22 MiB working set before opening the OS file dialog and around 68 MiB after it. These are workstation snapshots without AI inference.

Additional checkpoint: process-wide heavy-stage lease with race tests and fail-closed cleanup handling; admitted worker launch using fresh RAM/commit checks; Unicode child environment and working-directory isolation; embedded CPU-only Python module preflight exposed through CLI and a native interpreter-selection action. Twelve Rust tests, three stdlib Python tests and strict Clippy pass. The new interpreter-selection UI is compiled but has not been exercised because the user requested no further computer/UI automation.

Live CLI evidence: default Python 3.10.11 was correctly reported unsupported with missing dependencies; an installed Python 3.12.12 was accepted as a compatible interpreter but reported missing backend modules. Neither probe imported GPU/model packages or installed anything. A 128 MiB admitted synthetic worker was refused because host RAM was below the conservative reserve. A locked CPU-only dependency dry run succeeded and reported 211 packages for the inherited full backend; no environment was created and no packages were installed.

Native CPU checkpoint: the pinned OmniVoice C++/GGML runtime now builds as a 1,843,200-byte static CPU executable. Configuration/compilation were run inside a 768 MiB Job; recorded compilation peak commit accounting was 367,468,544 bytes. The staged runtime passed streamed checksum and CLI-contract preflight. Import inspection listed only KERNEL32, SHELL32 and ADVAPI32. This first engine can run without the full Python server or Torch; Python adapters remain optional for other engines.

Nineteen Rust tests, seven standalone Python tests, strict Clippy and live native standard-I/O/oversized-log checks pass. Worker stdin/stdout use an explicit inherited-handle whitelist; CPU rate limiting and post-exit log truncation are implemented and exercised. A fixed three-second native synthesis profiling path is implemented with atomic output publication, silent/non-finite sample rejection, explicit synthetic RIFF metadata and no overwrite of existing takes. Metadata marking is not AudioSeal.

The pinned Q4 model pair is being fetched explicitly into ignored local build storage, streamed/rate-limited and verified against source SHA-256 identities. Real synthesis remains unverified until the files finish downloading and the bounded fixture succeeds. GUI generation/cloning, reference-conditioned validation, original project/voice-library migration, long jobs and hardware certification remain incomplete.

Next milestone: finish model verification, measure actual native synthesis/cloning, then connect that bounded engine workflow to the native interface. Portable preview packaging is for development, not a completed application release. External backend requests still cannot bypass native resource safeguards. No UI automation is authorized during this work.
