# Native desktop decisions

## 2026-11-13: native Windows controls, Rust and GDI

The user's requirement is to remove Electron rather than harden or replace it with a WebView shell. The Windows-first prototype uses standard Windows controls and GDI through pinned windows-sys 0.61.2. This minimizes dependencies and avoids an application GPU-rendering stack. Native roles, text entry, local draft restore, file selection, playback, backend discovery and keyboard navigation were exercised in the rendered release build. OS compositing/dialog implementation is outside the application's renderer boundary. Further native Windows usability, scaling and accessibility validation remains necessary.

Python orchestration and existing AI engines remain reusable, but the native interface must not issue generation requests to an unconstrained external backend. Read-only discovery is a separate completed surface. The next implementation adds owned backend launch, shared admission and contained engine execution before enabling AI generation.

The Windows Job runner uses private-commit limits, kill-on-close and verified subtree drain. A disposable worker's oversized commit was refused, and timeout/cancellation cleanup passed. These tests do not certify GPU limits or real model capacity. Preserve system RAM and commit reserves independently, and never infer peak usage solely from model file size.

No application-wide version bump, upstream updater reuse, required cloud service or automatic model download is introduced by this checkpoint. Development work is committed/pushed to native-rust-desktop; source-baseline-0.5.8 remains the immutable imported reference.
