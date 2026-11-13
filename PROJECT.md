# Voice Studio native Rust fork

Repository: https://github.com/abusuraihsakhri/voice-studio

Goal: replace the Electron desktop with a native Rust UI using software rendering, preserving useful inference engines/backend behavior and adding conservative resource protection for modest PCs.

Current state: supplied VoiceStudio 0.5.8 source imported as a baseline; implementation specified in BUILD_SPEC.md. No native desktop build or low-resource validation yet.

Desktop migration direction in BUILD_SPEC.md overrides inherited upstream Electron-only rules. Electron remains reference source only. Existing licensing and attribution remain in force.

Workflow: commit and push coherent checkpoints; implementation on a development branch; preserve source-baseline-0.5.8. Actions initially disabled because inherited workflows target upstream Electron/mobile/releases; introduce appropriate native CI before enabling.

Next milestone: native software-renderer feasibility spike and API/data inventory, then backend ownership and resource admission.
