//! Central native synthetic-audio marking seam. Metadata is not an AudioSeal watermark.
use crate::audio;
use std::{fs::OpenOptions, io::Write, path::Path};
pub fn mark_synthetic(
    source: &Path,
    destination: &Path,
    provenance: &serde_json::Value,
) -> Result<audio::WaveInfo, String> {
    let mut bytes = audio::load_preview(source)?;
    let info = audio::inspect_wave(&bytes)?;
    if info.peak_abs == 0. {
        return Err("Engine produced silent audio; no completed take was published.".into());
    }
    if provenance.get("synthetic") != Some(&serde_json::Value::Bool(true)) {
        return Err("Synthetic provenance metadata is required.".into());
    }
    let mut text = serde_json::to_vec(provenance).map_err(|_| "Cannot encode audio provenance.")?;
    if text.len() > 8192 {
        return Err("Audio provenance exceeds its bounded size.".into());
    }
    text.push(0);
    let mut chunk = Vec::from(&b"INFOICMT"[..]);
    chunk.extend((text.len() as u32).to_le_bytes());
    chunk.extend(&text);
    if text.len() % 2 != 0 {
        chunk.push(0);
    }
    bytes.extend(b"LIST");
    bytes.extend((chunk.len() as u32).to_le_bytes());
    bytes.extend(chunk);
    if bytes.len() as u64 > audio::MAX_PREVIEW_BYTES {
        return Err("Marked audio exceeds the native take-size budget.".into());
    }
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    audio::inspect_wave(&bytes)?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "Cannot create a marked take without overwriting an existing file.")?;
    target
        .write_all(&bytes)
        .map_err(|_| "Cannot write marked audio.")?;
    target
        .sync_all()
        .map_err(|_| "Cannot flush marked audio.")?;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_mark_preserves_samples_and_refuses_overwrite() {
        let dir = std::env::temp_dir().join(format!(
            "voice-studio-provenance-test-{}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let source = dir.join("raw.wav");
        let target = dir.join("marked.wav");
        let raw = audio::fixture_wave(&[0, 16384, -16384, 0]);
        std::fs::write(&source, &raw).unwrap();
        let info = mark_synthetic(
            &source,
            &target,
            &serde_json::json!({"synthetic":true,"engine":"fixture"}),
        )
        .unwrap();
        assert_eq!(info.frames, 4);
        assert_eq!(info.peak_abs, 0.5);
        let marked = std::fs::read(&target).unwrap();
        assert_eq!(&raw[8..], &marked[8..raw.len()]);
        assert!(marked.windows(16).any(|w| w == b"\"synthetic\":true"));
        assert_eq!(audio::inspect_wave(&marked).unwrap().frames, 4);
        assert!(mark_synthetic(&source, &target, &serde_json::json!({"synthetic":true})).is_err());
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(target).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn silent_generated_take_is_not_published() {
        let source = std::env::temp_dir().join(format!(
            "voice-studio-silence-test-{}.wav",
            std::process::id()
        ));
        let destination = source.with_extension("marked.wav");
        std::fs::write(&source, audio::fixture_wave(&[0, 0, 0])).unwrap();
        assert!(
            mark_synthetic(
                &source,
                &destination,
                &serde_json::json!({"synthetic":true})
            )
            .is_err()
        );
        assert!(!destination.exists());
        std::fs::remove_file(source).unwrap();
    }
}
