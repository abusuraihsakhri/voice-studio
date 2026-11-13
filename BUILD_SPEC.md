# Voice Studio: native Rust desktop migration

Status: implementation in progress, 2026-11-13. A native Rust/Win32 desktop preview has been built and rendered. Backend/model execution integration, real synthesis and full acceptance gates remain incomplete. See docs/native-migration.md for the evidence and feature inventory.

## Binding direction for this fork

The user requires replacing Electron. The new application must have a native Rust desktop interface with a verified software-rendering path. Electron, Chromium and embedded browser/webview shells are excluded from the new desktop runtime. Keep useful Python orchestration and existing inference engines in isolated processes. GPU inference is independent of UI rendering and optional.

This supersedes the inherited Electron-only instructions in AGENTS.md and CLAUDE.md, and the earlier plan to harden Electron first. Original Electron code is retained as a reference until feature migration is validated; it is not the destination application.

## Preservation and boundaries

- Preserve the imported source baseline in Git, including attribution, LICENSE and LICENSE-NOTICE.md. The baseline tag is a source snapshot, not a product release.
- Preserve existing voices, projects and model caches. Development uses a separate data directory; test migration on copies first.
- Do not rewrite model implementations simply to eliminate Python. Reuse native or Python workers where they provide validated behavior.
- Do not introduce automatic downloads, uploads, telemetry or account dependencies for the new desktop.
- Do not claim every engine fits low-resource hardware. Expose unsupported capacity clearly.
- Keep mobile/web code as reference; simultaneous mobile/web redevelopment is outside the initial Windows desktop scope.
- Disable inherited GitHub Actions initially: upstream workflows include Electron/mobile/release/deployment operations not appropriate to this new repository. Replace with targeted Rust/backend CI before enabling Actions. No upstream release feeds or signing identities are reused.

## Source findings

| Existing surface | Migration decision |
|---|---|
| `electron/src/main/index.ts`, `ipc.ts`, preload and renderer | Inventory behavior, then replace with native Rust views and desktop services. |
| `electron/src/main/backend.ts` | Port necessary startup, readiness, exit classification and runtime discovery; do not assume the helper alone implements these. |
| `backend/services/memory_budget.py` | Advisory-only today; implement enforceable admission. Its GPU probe imports Torch, unsuitable for independent lightweight monitoring. |
| `backend/services/engine_memory.py` | Retain eviction logic, extend discipline beyond TTS, and verify release before admitting the next stage. |
| `backend/services/model_manager.py` | Existing pool can select multiple CUDA workers; conservative policy permits one heavy stage. |
| `backend/core/job_queue.py` | Add bounded pending work and terminal retention without breaking active-job semantics. |
| `backend/services/subprocess_backend.py` | Reuse sidecar protocol, cancellation/timeouts and idle cleanup. |
| `backend/core/contained_subprocess.py` | Reuse process-tree ownership; extend Windows Job accounting/limits and validate nested ownership. |
| `native/desktop-bridge/` | Evaluate reusable shortcut/clipboard/cleanup modules individually; it is not a native application shell. |
| `backend/engines/omnivoice_gguf/` | Candidate small native engine; benchmark actual peaks and output quality before making a default. |

The imported source identifies as 0.5.8. No diagnosis of the current system crashes or low-RAM performance certification has been completed.

## Architecture

```text
Native Rust desktop UI (software renderer)
  -> Rust application state, resource admission and stage lease
  -> native CPU inference worker for the first OmniVoice workflow
  -> optional isolated Python adapters for engines that require them
  -> verified model files and staged/marked output files

Existing authenticated local API: optional compatibility/discovery surface;
the first native CPU workflow does not require the full Python server.
```

Do not expose engine execution directly from widget callbacks. UI work stays responsive while jobs execute. Worker death must leave the UI and project state usable. One ownership hierarchy controls shutdown; avoid competing supervisors or PID-name-based termination.

Renderer decision: native Windows controls plus GDI through pinned windows-sys 0.61.2, following the rendered feasibility spike, native accessibility roles, file-dialog checks and bounded PCM playback. No embedded browser or application GPU renderer is introduced. Windows desktop composition and system dialogs remain OS responsibilities. Portable packaging and continued DPI/accessibility checks are required; no cross-platform UI parity is claimed.

Use existing engine implementations and data contracts wherever possible. Inspection found that the inherited server starts model warm-ups, its TTS registry imports Torch, and its locked CPU installation includes 211 packages. The first native CPU workflow therefore invokes the pinned native OmniVoice runtime directly, avoiding server startup and Torch for that workflow. Other engines may retain isolated Python adapters; optional backend compatibility still requires authentication, ownership, bounded transport and cancellation. No new unauthenticated network service is introduced by native inference. This is an evidence-based architecture refinement, not a commitment to discard supported engines or project data.

## Native feature inventory and migration order

1. Window lifecycle, settings, backend status, local diagnostics and explicit CPU/GPU choice.
2. Existing voice library, reference-file selection, text editor, engine selection, generation, progress, cancellation, playback and export.
3. Project opening/saving and migration checks against copied existing data.
4. Queued narration/batch with disk-backed chunks and resume.
5. Transcription and staged dubbing, followed by additional engines and remaining desktop integrations.

For every feature record the existing API, file/data schema, error states, relevant tests and native implementation status. Do not claim full parity at the first working synthesis. Track shortcut/tray, recording permissions, drag/drop, update behavior, waveform display and file authorization separately.

## Resource safety

Implement a versioned conservative resource profile. Fields: RAM/commit reserves, worker private-commit cap, GPU reserve where measurable, one heavy-stage concurrency, pending queue limit, idle unload and bounded retry settings. Validate configuration and show effective environment overrides.

Admission runs under a single shared lease, spanning eviction through worker exit. All heavy operations, including API requests, previews, ASR, synthesis, diarization, separation, SFX and dubbing stages, must use it. Inventory bypasses before implementing it. Nested stages carry an existing lease token rather than reacquire and deadlock. Downloads/installers do not hold an inference lease while installing.

Check RAM and system commit even when VRAM is plentiful. Shared/integrated GPU memory is not a separate fictitious capacity pool. Unknown probes are not unlimited capacity. Estimate peak loading/generation from measured engine/model/runtime/workload records, not model download size.

Starting tuning hypothesis: reserve at least max(2 GiB, 25% host RAM), one heavy stage, smallest suitable installed model. This is not a validated 8 GB support guarantee. Profile representative 8 GB CPU-only, 12–16 GB and small-GPU systems. Decline a job when its measured envelope does not fit; CPU fallback also needs RAM admission.

Windows Job private-commit limits protect worker allocation boundaries but do not cap GPU VRAM or all physical working-set usage. Apply limits before resuming child execution; failure to contain an engine prevents conservative-mode launch. Keep control backend and UI outside the worker's tight limit. Catch/report allocation failures; kill/reap stuck owned subtrees after a cancellation grace period.

Use an independent lightweight Rust sampler for host memory/commit and process accounting without importing Torch or initializing CUDA. GPU probes run out of process with deadlines and are optional. Normal/Warning/Critical/Recovering states need hysteresis. Block starts and evict idle workers under warning; cancel then terminate offending owned workers at critical pressure. Polling cannot catch every allocation spike: combine it with OS enforcement.

Remaining heavy in-process Python paths must move behind existing sidecar contracts or be unavailable in conservative mode until contained. Do not assert full protection merely because some engines already run separately.

No automatic repeat of OOM, access-violation or driver-failing jobs. Offer explicit alternatives after cleanup and re-admission. Bound backend restart attempts and persist a crash-loop marker. Distinguish cancellation, capacity refusal, native fault, GPU OOM, timeout and model/runtime errors.

## Long media and durable work

Queue file references and metadata, not decoded audio. Bound pending jobs, retained history, event/log buffers, waveform previews and audio decode buffers. Stream or segment media where engine semantics allow it. Do not arbitrarily truncate voice references or change speaker/context behavior.

Write completed segments atomically with input/config/model identity. Resume only when identities match. Use existing project persistence before adding another database. Final assembly must avoid loading an entire long recording into RAM. Cancellation must actually stop the worker before releasing its lease or starting replacement work.

## Commit-sized implementation batches

| Batch | Work | Completion evidence |
|---|---|---|
| 0 | Preserve baseline, write fork instructions and specification | Remote commit/tag verified; clean working tree. |
| 1 | Inventory API/IPC/data and validate native software-rendered toolkit | Small Rust window, no Chromium/webview processes, renderer path documented; keyboard/file/audio checks. |
| 2 | Native shell + backend supervisor/client | Owned backend startup/readiness/auth/shutdown and crash-loop tests; original data untouched. |
| 3 | Resource policy, stage lease and worker limits | Deterministic admission/race tests and disposable allocation failure without parent failure. |
| 4 | Native voice cloning/TTS workflow | Real reference/text -> nonzero playable WAV; cancellation, export and repeated runs measured. |
| 5 | Projects, batch/narration and chunk resume | Copied-data compatibility and interruption/restart checks. |
| 6 | ASR/dubbing/additional engines + packaging | Explicit feature matrix and hardware acceptance report; side-by-side installer. |

Commit after each coherent tested increment and push checkpoints to a development branch. Keep main as the preserved reviewed baseline/spec until implementation is ready to integrate. Tag meaningful verified milestones. Never commit credentials, recordings, model weights, installed runtimes or private user data.

## Validation gates

- Unit tests: resource arithmetic, unknown telemetry, malformed settings, RAM/VRAM mismatch, shared GPU accounting, commit pressure, queue bounds and lease cleanup.
- Concurrency: simultaneous requests cannot reserve the same remaining capacity; nested stages do not deadlock; busy workers cannot be evicted underneath inference.
- Containment: disposable child exceeds cap, hangs, fills pipes or exits unexpectedly; UI/control backend survives and no descendants remain.
- UI: real software-renderer verification, keyboard navigation/focus, readable scaling, file selection, playback and recording where implemented.
- Functional: ten short syntheses, twenty engine switches where supported, cancellation during load/generation, long narration and restart/resume. Observe trends rather than mislabel allocator caches as leaks.
- Data: existing project/voice samples open from copies; originals and model cache files remain unchanged.
- Build: Rust formatting/check/tests and targeted backend tests. Pinned toolkit/runtime versions and reproducible Windows packaging. No inherited Electron build as a gate for the new application.

Record process-tree private commit and working set, host available RAM/commit, GPU dedicated/shared use when supported, output duration/sample rate, latency, responsiveness and orphan count. Synthetic constrained-worker tests supplement physical hardware testing; they do not certify real low-end GPUs.

Full machine crash prevention cannot be guaranteed. Driver/kernel faults and unrelated software remain outside application containment. Report unsupported engines and untested hardware explicitly.

## References

- Windows Job accounting/limits: https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information
- Ownership and nested Jobs: https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects

## Next action

Continue on native-rust-desktop with owned backend/runtime integration and real synthesis behind the resource-safety boundary. Preserve the verified native window and worker primitives; do not enable an uncontained generation shortcut through an external backend. Complete remaining acceptance gates before calling the goal achieved.
