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

Provide managed backend/runtime selection and authenticated owned launch while preserving the existing environment and data. Move generation behind the native resource lease and contained workers, then deliver a real voice-cloning/TTS output. Do not report this preview as the completed goal.
