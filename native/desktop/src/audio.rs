//! Bounded native PCM preview. Compressed and oversized media need a streaming decoder.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
pub const MAX_PREVIEW_BYTES: u64 = 32 * 1024 * 1024;
pub fn load_preview(path: &Path) -> Result<Vec<u8>, String> {
    let source = File::open(path).map_err(|_| "Choose a readable WAV file.")?;
    let size = source
        .metadata()
        .map_err(|_| "Cannot inspect audio file.")?
        .len();
    if !(44..=MAX_PREVIEW_BYTES).contains(&size) {
        return Err("Native preview supports WAV files up to 32 MiB. Larger media will require streaming playback.".into());
    }
    let mut bytes = Vec::new();
    source
        .take(MAX_PREVIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the selected WAV.")?;
    if bytes.len() as u64 != size {
        return Err("Audio file changed while loading; retry with a stable file.".into());
    }
    let mut file = std::io::Cursor::new(&bytes);
    let mut header = [0u8; 12];
    file.read_exact(&mut header)
        .map_err(|_| "Truncated WAV header.")?;
    if &header[..4] != b"RIFF"
        || &header[8..] != b"WAVE"
        || u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8 != size
    {
        return Err("Select a complete standard RIFF/WAV file.".into());
    }
    let mut position = 12u64;
    let mut format = None;
    let mut data = None;
    for _ in 0..128 {
        if position == size {
            break;
        }
        if position + 8 > size {
            return Err("Truncated WAV chunk.".into());
        }
        file.seek(SeekFrom::Start(position))
            .map_err(|_| "Cannot read WAV.")?;
        let mut chunk = [0u8; 8];
        file.read_exact(&mut chunk)
            .map_err(|_| "Truncated WAV chunk.")?;
        let length = u32::from_le_bytes(chunk[4..].try_into().unwrap()) as u64;
        let end = position + 8 + length;
        if end > size {
            return Err("WAV chunk exceeds the file size.".into());
        }
        if &chunk[..4] == b"fmt " {
            if format.is_some() || length < 16 {
                return Err("Invalid WAV format chunk.".into());
            }
            let mut fmt = [0u8; 16];
            file.read_exact(&mut fmt)
                .map_err(|_| "Truncated WAV format.")?;
            let kind = u16::from_le_bytes(fmt[..2].try_into().unwrap());
            let channels = u16::from_le_bytes(fmt[2..4].try_into().unwrap());
            let rate = u32::from_le_bytes(fmt[4..8].try_into().unwrap());
            let align = u16::from_le_bytes(fmt[12..14].try_into().unwrap());
            let bits = u16::from_le_bytes(fmt[14..].try_into().unwrap());
            if !matches!(channels, 1 | 2)
                || !(8000..=192000).contains(&rate)
                || !((kind == 1 && matches!(bits, 16 | 24 | 32)) || (kind == 3 && bits == 32))
                || align != channels * bits / 8
            {
                return Err("Preview requires uncompressed mono/stereo PCM WAV (16/24/32-bit) or 32-bit float WAV.".into());
            }
            format = Some(align as u64);
        }
        if &chunk[..4] == b"data" {
            if data.is_some() || length == 0 {
                return Err("Invalid WAV audio-data chunk.".into());
            }
            data = Some(length);
        }
        position = end + (length % 2);
    }
    if position != size {
        return Err("Unsupported WAV chunk layout.".into());
    }
    match (format, data) {
        (Some(align), Some(length)) if length % align == 0 => Ok(bytes),
        _ => Err("WAV audio data is missing or misaligned.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_pcm_and_rejects_truncation() {
        let mut wav = Vec::from(&b"RIFF"[..]);
        wav.extend(38u32.to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(24000u32.to_le_bytes());
        wav.extend(48000u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend(2u32.to_le_bytes());
        wav.extend([0, 0]);
        let path = std::env::temp_dir().join(format!(
            "voice-studio-audio-test-{}.wav",
            std::process::id()
        ));
        std::fs::write(&path, &wav).unwrap();
        assert!(load_preview(&path).is_ok());
        wav.pop();
        std::fs::write(&path, &wav).unwrap();
        assert!(load_preview(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
