# Independent native Rust fork

Follow the user's explicit directions and BUILD_SPEC.md. PROJECT.md records current status; DECISIONS.md records architecture decisions.

- Target a native Rust Windows desktop using standard controls/GDI, with no Electron, Chromium or WebView desktop runtime.
- Preserve original data and third-party source-license/copyright notices. Do not imply affiliation, endorsement, sponsorship or commercial support by external authors or organizations.
- Reuse validated engines where appropriate; keep inference in owned resource-limited workers. Preserve system RAM/commit headroom; GPU inference is optional and independent of UI rendering.
- No model downloads, uploads, telemetry or account dependencies without the user's authorized scope.
- Keep edits focused. Do not revert unrelated user work or overwrite raw data.
- Commit and push coherent checkpoints to native-rust-desktop. Do not rewrite history or delete the source baseline.
- Run relevant command-line checks. Do not open windows, use UI automation, or take over mouse/keyboard unless the user permits it again.
- Report actual evidence and unfinished functionality honestly. A preview build is not feature parity or low-resource hardware certification.
