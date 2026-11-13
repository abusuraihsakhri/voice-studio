# VoiceStudio — License Notice

## Abbreviation

AGPL-3.0-only

## Notice

Copyright 2024-present Palash Debnath and VoiceStudio contributors.

VoiceStudio is **free and open-source software, licensed under the GNU
Affero General Public License, Version 3 (AGPL-3.0)**. You are free to use,
copy, modify, and redistribute it. That **includes commercial and internal
business use** of the application itself. Model weights, tokenizers, and other
third-party assets retain their own terms; this application license does not
grant or summarize rights under those separate terms.

Because this is the **Affero** GPL, one additional obligation applies: if you
modify VoiceStudio and make that modified version available to others over
a network, you must also offer those users the complete corresponding source
code of your modified version under these same AGPL-3.0 terms. See the full
text in [`LICENSE`](LICENSE).

This repository is an independent fork. The original authors, companies and sponsors do not maintain, endorse or provide commercial support for it. Copyright and third-party license notices identify source provenance, not an affiliation. No commercial license or paid-service offer is made here.

### Scope

These terms cover the VoiceStudio application — the Electron desktop and web
app (`electron/`), its native desktop helper (`native/`), the FastAPI backend
(`backend/`), the unified command package (`cli/`), and supporting build / packaging files (`scripts/`, `deploy/`,
`.github/`). Electron components adapted from T3 Code keep their MIT notice in
`electron/T3CODE-LICENSE.txt`.

The bundled `omnivoice/` Python package — the underlying TTS model by Han Zhu —
is **separately licensed under Apache License 2.0** by its upstream authors and
is not relicensed here. Apache License 2.0 is compatible with, and may be
combined under, the GNU AGPL-3.0. See `pyproject.toml`.

Downloaded model weights are not relicensed by VoiceStudio. The default
`k2-fsa/OmniVoice` model card identifies its code as Apache-2.0 and pretrained
weights as CC-BY-NC. Its `audio_tokenizer/LICENSE` contains separate Boson
Higgs Audio 2 and Meta Llama community terms. A commercial license for
VoiceStudio-owned code does not replace any of those terms.

The maintained About panel displays selected model credits and required literal
Higgs Audio and Llama attribution text. See [model credit sources](docs/model-credits.md)
for evidence and remaining gaps. These visible credits do not establish complete
notice compliance or permission for a particular use.

The [model licence records](backend/config/model_licenses.json) distinguish
inspected non-commercial terms from unreviewed upstream metadata. A false
commercial-use flag includes unresolved review; it is not a claim that every
listed model forbids commercial use. No commercial clearance is asserted by
the initial inventory.

The Model Manager's versioned notices distinguish commercial model use,
commercial output use, redistribution and voice/recording consent. An unknown
assessment is not permission and is not a finding that all uses are prohibited.
Evidence for one revision or conversion does not automatically cover another.

The optional [reviewed-file workflow](docs/licensing/model-review-workflow.md)
records a local acknowledgement of the displayed notice and exact evidence.
It does not accept agreements with a provider, grant provider access, grant
commercial rights, verify voice consent or add a waiver. It does not alter
rights granted by the application's AGPL licence. Existing engine settings and
ordinary download/first-use paths remain separate and unchanged; no production
model currently has the complete evidence needed for this reviewed workflow.

Third-party dependencies retain their own licenses. See `bun.lock`, `uv.lock`,
and `native/desktop-bridge/Cargo.lock` for the resolved set.

The desktop's terminal command bundles a Python interpreter using PyInstaller
(build pin in `electron/cli-bundle.mjs`). Interpreter and bootloader notices ship
under `resources/cli/_internal/licenses` (`Contents/Resources` on macOS).

The locked PyAV 15.1.0 wheels bundle FFmpeg and x264/x265 libraries. PyAV's source
licence alone does not describe those binaries' terms. The
[wheel audit](docs/licensing/pyav-15.1.0-audit.md) records their hashes, build flags,
upstream licence-label patch, and unresolved redistribution requirements.

### Reference

The full canonical text of the GNU Affero General Public License, Version 3 is
reproduced verbatim in [`LICENSE`](LICENSE). The authoritative copy lives at
<https://www.gnu.org/licenses/agpl-3.0.txt>.

> **Why this notice is a separate file:** `LICENSE` must contain the verbatim
> AGPL-3.0 text and nothing else, so GitHub's license detection (and the
> corporate license scanners that gate adoption) can identify it as
> `AGPL-3.0-only` rather than falling back to "Other" / `NOASSERTION`.
