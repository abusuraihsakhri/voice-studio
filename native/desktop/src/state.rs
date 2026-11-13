//! Isolated native draft storage; never writes an existing OmniVoice profile.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub text: String,
    pub audio: String,
}
#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    version: u32,
    draft: Draft,
}
pub fn draft_path() -> Result<PathBuf, String> {
    let root = std::env::var_os("VOICE_STUDIO_NATIVE_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("VoiceStudioNative"))
        })
        .ok_or("Native data directory is unavailable.")?;
    if !root.is_absolute() {
        return Err("Native data directory must be absolute.".into());
    }
    Ok(root.join("draft.json"))
}
pub fn load(path: &Path) -> Result<Draft, String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Draft::default()),
        Err(_) => return Err("Cannot read the native draft.".into()),
    };
    let mut bytes = Vec::new();
    file.take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the native draft.")?;
    if bytes.len() > 256 * 1024 {
        return Err("Native draft exceeds the supported size. Original file preserved.".into());
    }
    let envelope: Envelope = serde_json::from_slice(&bytes)
        .map_err(|_| "Native draft is not recognized. Original file preserved.")?;
    if envelope.version != 1 || envelope.format != "voice-studio-native-draft" {
        return Err("Native draft format is incompatible. Original file preserved.".into());
    }
    validate(&envelope.draft)?;
    Ok(envelope.draft)
}
fn validate(draft: &Draft) -> Result<(), String> {
    if draft.text.encode_utf16().count() > 16000
        || draft.audio.encode_utf16().count() > 32767
        || draft.text.contains('\0')
        || draft.audio.contains('\0')
    {
        return Err("Draft exceeds native text/path limits.".into());
    }
    Ok(())
}
pub fn save(path: &Path, draft: &Draft) -> Result<(), String> {
    validate(draft)?;
    let parent = path.parent().ok_or("Invalid draft location.")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create the native data directory.")?;
    let bytes = serde_json::to_vec(&Envelope {
        format: "voice-studio-native-draft".into(),
        version: 1,
        draft: draft.clone(),
    })
    .map_err(|_| "Cannot encode the native draft.")?;
    let temporary = parent.join(format!(".draft-{}.tmp", std::process::id()));
    // create_new prevents following a pre-existing temporary path or clobbering another writer.
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| "Cannot stage draft; existing draft was not changed.")?;
    let result = (|| {
        let write_result = (|| {
            file.write_all(&bytes).map_err(|_| "Cannot write draft.")?;
            file.sync_all().map_err(|_| "Cannot flush draft.")
        })();
        drop(file);
        write_result?;
        fs::rename(&temporary, path)
            .map_err(|_| "Cannot replace draft; existing draft was preserved.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_roundtrip_and_preserved_invalid_input() {
        let dir =
            std::env::temp_dir().join(format!("voice-studio-state-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("draft.json");
        let draft = Draft {
            text: "Native draft — മലയാളം".into(),
            audio: "C:\\voices\\reference.wav".into(),
        };
        save(&path, &draft).unwrap();
        assert_eq!(load(&path).unwrap(), draft);
        let mut invalid = draft.clone();
        invalid.text = "x".repeat(16001);
        assert!(save(&path, &invalid).is_err());
        assert_eq!(load(&path).unwrap(), draft);
        fs::write(&path, b"{\"version\":999}").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{\"version\":999}");
        fs::remove_dir_all(dir).unwrap();
    }
}
