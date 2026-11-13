# Native desktop decisions

## 2026-11-13: native Windows controls, Rust and GDI

The user's requirement is to remove Electron rather than harden or replace it with a WebView shell. The Windows-first prototype uses standard Windows controls and GDI through pinned windows-sys 0.61.2. This minimizes dependencies and avoids an application GPU-rendering stack. Native roles, text entry, local draft restore, file selection, playback, backend discovery and keyboard navigation were exercised in the rendered release build. OS compositing/dialog implementation is outside the application's renderer boundary. Further native Windows usability, scaling and accessibility validation remains necessary.

Python orchestration and existing AI engines remain reusable, but the native interface must not issue generation requests to an unconstrained external backend. Read-only discovery is a separate completed surface. The next implementation adds owned backend launch, shared admission and contained engine execution before enabling AI generation.

The Windows Job runner uses private-commit limits, kill-on-close and verified subtree drain. A disposable worker's oversized commit was refused, and timeout/cancellation cleanup passed. These tests do not certify GPU limits or real model capacity. Preserve system RAM and commit reserves independently, and never infer peak usage solely from model file size.

No application-wide version bump, upstream updater reuse, required cloud service or automatic model download is introduced by this checkpoint. Development work is committed/pushed to native-rust-desktop; source-baseline-0.5.8 remains the immutable imported reference.

## 2026-11-13: direct native CPU inference before the full Python server

Static inspection and a locked dry run found 211 CPU-backend packages, a top-level Torch import in the TTS registry, and background model warm-ups in server startup. Requiring that entire server for the first native workflow would preserve avoidable installation and memory overhead. Reuse the existing native OmniVoice runtime directly under Rust's resource/ownership boundary. Keep Python adapters for engines that need them and preserve existing data contracts.

The CPU runtime and GGML submodule are pinned to 040c8b344d8c670ce1475194751d119b5ef82c78 and d5152bf4348453068aa8de0d479caef40a613b9d. A static baseline build disables GPU backends, dynamic backend loading, AVX/AVX2/AVX512, FMA/F16C and SSE4.2. The actual binary imports only KERNEL32, SHELL32 and ADVAPI32. Third-party license notices ship beside it. The Rust engine preflight verifies the manifest, streamed SHA-256 and CLI contract without loading models.

Pinned Q4 model identities come from the model repository's blob metadata at revision 361609388ae572a820d085185bbbe2a2aac4b30e. The explicit developer fetcher streams, rate-limits, resumes identified partial files and verifies sizes/checksums before publishing; model weights stay outside Git and native preview ZIPs. Model terms remain separate from the application/runtime licenses.

The first real inference experiment is deliberately a fixed, three-second, seed-42, 32-step CPU fixture with a 1 GiB worker commit ceiling, 25% CPU Job cap, minimum 2 GiB host reserve, timeout, explicit standard-handle whitelist and bounded logs. It establishes evidence before unrestricted native generation is enabled; it is not certification of arbitrary text, languages, reference audio or low-end hardware. Ordinary unknown-envelope admission remains blocked.

Native generated WAVs pass a central marking seam that preserves audio samples and adds explicit synthetic RIFF metadata. This metadata can be stripped and is not an invisible AudioSeal watermark or a claim of legal compliance. Existing Python watermark adapters remain reference for future integration; this behavior difference is documented rather than hidden.
