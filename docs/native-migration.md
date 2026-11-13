# Native desktop migration inventory

The original desktop remains reference source; the replacement is `native/desktop/`.

## Renderer decision

Use Windows standard controls plus GDI for the Windows-first native build, through pinned `windows-sys` bindings. This avoids introducing an application GPU-rendering engine or embedded browser. Standard edit/button/static controls expose native accessibility roles. Native file dialogs/playback are OS integrations. This is a Windows UI implementation, not cross-platform desktop parity.

Observed in the first rendered release prototype on this workstation: approximately 19–21 MiB working set and 3–4 MiB private commit before opening a file dialog; approximately 64–67 MiB working set after the Windows file dialog loaded shell components. These are snapshots, not performance guarantees. No model was loaded. The executable was approximately 0.4 MiB before subsequent checkpoint changes; exact artifact size/hash belongs in its build manifest.

## API and desktop migration surface

| User feature | Source contract | Native status / next gate |
|---|---|---|
| Resource status | Independent Win32 APIs | Implemented; no GPU probes. Physical low-RAM validation remains. |
| Backend readiness | `GET /health`, `backend/main.py`; Electron `backend.ts` | Read-only loopback client tested. Owned Python launch/install/readiness still required. |
| Engine inventory | `GET /engines`, `api/routers/engines.py`; family `backends` arrays | Read-only summary client. Selection UI and compatibility gates pending. |
| Voice library | `GET /profiles`, profile CRUD/audio routes | Read-only count. Native list/details/create/update and file uploads pending. |
| TTS/cloning | multipart `POST /generate`, `api/routers/generation.py`; WAV response with audio/seed/duration headers | Pending resource-admitted owned engine workflow. Preserve synthetic-audio marking and complete metadata. |
| Local text draft | New versioned native draft envelope | Implemented in separate native storage; not an original project migration. |
| Projects | `api/routers/projects.py` list/get/create/update/delete and `project_bundles.py` | Pending; inspect schema and test copies before writing. |
| Audio preview | Original renderer media consumers; native RIFF/PCM validation | Bounded native playback implemented; waveform and streaming long media pending. |
| Batch/narration | `api/routers/batch.py`, `audiobook.py`, longform render/resume routes | Pending; retain existing durable job model, disk-backed chunks and cancellation. |
| Dubbing | `dub_core.py`, `dub_translate.py`, `dub_generate.py`, `dub_export.py` | Pending staged native views and global resource lease. |
| ASR/dictation | Existing ASR adapters, routers and `native/desktop-bridge/` | Pending route/event inventory, capture, ownership and permissions. |
| Shortcuts/tray | `native/desktop-bridge/`, Electron main | Pending selectively reusable modules and native shell integration. |
| File authorization | `ipc.ts`, `core/path_authorization.py` | Native picker implemented; backend capability bridge pending. Do not bypass existing checks. |
| Recovery | `subprocess_backend.py`, `contained_subprocess.py`, Electron journals | Native worker primitive tested. Model-worker integration, bounded restart and persisted jobs pending. |
| Packaging | New `scripts/build-native.ps1` | Portable development preview only. Installer, signing and isolated update identity pending. |

## Protection boundaries

The native worker runner applies a Windows Job private-commit ceiling and kills/drains owned children on failure/exit. It is currently exercised by a disposable allocation probe, not connected to AI generation. The native resource arithmetic rejects unknown envelopes and preserves reserves. Neither primitive constitutes completed global orchestration: shared leases, all-entry-point admission, GPU safety and remaining in-process isolation are still pending.

Existing external backends discovered through the UI are not owned or constrained by the native app. Discovery must not offer an unsafe generation path as a shortcut around this boundary. UI rendering independence from the GPU does not imply that inference or Windows' compositor is GPU-free.

## Validation checkpoint

Executed: native build and optimized build; resource arithmetic tests; local HTTP fixtures for authentication, route/credential validation, redirect refusal and declared-size refusal; bounded Unicode draft persistence; PCM/truncated-WAV validation; disposable allocation-limit test; rendered release window and text entry; actual atomic draft file observed; keyboard mnemonic opened/cancelled a native file picker. Strict Clippy was clean at the preceding checkpoint and must be rerun for final packaging.

Also executed after the latest changes: final native layout/accessibility inspection; Tab movement from port to Connect; distinct keyboard mnemonics; local discovery fixture displayed 0 voices/1 test-only engine; native picker selected a Python-wave-generated silent PCM WAV; playback API success and Stop/buffer release displayed; draft text restored after reopening. Allocation, timeout and cancellation probes passed with confirmed Job subtree drain. A live budget check refused a 1 GiB envelope when host headroom was insufficient. One observed native process after the OS file dialog used 71,618,560 bytes working set and 16,318,464 bytes private commit; no modules matching d3d/dxgi/vulkan/opengl/chrome/webview were observed. This is a module-name inspection, not a universal GPU-driver guarantee.

The nine Rust unit tests and strict Clippy passed before the final Ctrl+A compatibility change; final packaging reruns them. Packaged-folder execution and the final Ctrl+A behavior must be rechecked. No actual AI output or 8 GB hardware certification exists yet. The fixture is intentionally not an inference backend and cannot substantiate model compatibility.

## Immediate next milestone

### Command-line-only checkpoint after the UI pause

The native admitted-worker function now acquires a process-wide lease, samples host memory after acquiring it, checks an explicit memory envelope, applies the worker commit ceiling, and releases the lease only after cleanup confirmation. An unresolved cleanup poisons the gate until process restart. Race tests demonstrate that eight competing requests admit one holder. This protects this launch API; legacy/external backend routes are not yet wired through it and AI generation remains disabled in the native UI.

Worker launches support validated Unicode environment overrides and an explicit absolute working directory. The embedded runtime preflight runs the selected Python with `-I`, CPU/offline environment settings, a 128 MiB private-commit cap and a 15-second execution budget. Reports are bounded to 32 KiB, do not contain executable paths or credentials, are parsed against the discovery contract, and are removed from a freshly created temporary directory. No model package is imported by the probe.

CLI observations: Python 3.10.11 correctly returned unsupported; an already installed Python 3.12.12 passed the version check but had no backend dependencies. Both reported no imported GPU modules. Admission of the 128 MiB synthetic probe was refused on current host RAM pressure. A locked CPU dependency dry run found the inherited backend would install 211 packages, including many optional engine stacks; it changed no lockfile/environment and performed no installation. Audit lazy imports/dependency profiles before choosing a smaller control-plane environment; do not invent a minimal dependency set or silently disable supported engines.

Twelve Rust unit tests, three standalone Python tests and strict Clippy passed. The interpreter-picker action is implemented and compiles; it was deliberately not opened or automated. A functioning model worker, real synthesis and low-end hardware certification remain outstanding.

Provide managed backend/runtime selection and authenticated owned launch while preserving the existing environment and data. Move generation behind the native resource lease and contained workers, then deliver a real voice-cloning/TTS output. Do not report this preview as the completed goal.
