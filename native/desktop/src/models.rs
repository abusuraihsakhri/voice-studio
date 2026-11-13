use crate::integrity;
use serde::Deserialize;
use std::{io::Read, path::Path};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema_version: u32,
    pub repository: String,
    pub revision: String,
    pub files: Vec<ModelFile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}
pub fn catalog() -> Result<Catalog, String> {
    let value: Catalog = serde_json::from_str(include_str!("../models/omnivoice-q4.json"))
        .map_err(|_| "Invalid native model catalog.")?;
    if value.schema_version != 1
        || value.repository != "Serveurperso/OmniVoice-GGUF"
        || value.revision != "361609388ae572a820d085185bbbe2a2aac4b30e"
        || value.files.len() != 2
        || value.files[0].name != "omnivoice-base-Q4_K_M.gguf"
        || value.files[1].name != "omnivoice-tokenizer-Q4_K_M.gguf"
    {
        return Err("Unsupported native model catalog.".into());
    }
    Ok(value)
}
pub fn verify(directory: &Path) -> Result<Catalog, String> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("Select an existing absolute model directory.".into());
    }
    let value = catalog()?;
    for specification in &value.files {
        let path = directory.join(&specification.name);
        integrity::verify_file(&path, specification.bytes, &specification.sha256)?;
        let mut file =
            std::fs::File::open(path).map_err(|_| "Cannot inspect verified GGUF header.")?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header)
            .map_err(|_| "Truncated GGUF header.")?;
        if &header[..4] != b"GGUF"
            || !matches!(u32::from_le_bytes(header[4..].try_into().unwrap()), 2 | 3)
        {
            return Err("Model file does not use a supported GGUF format.".into());
        }
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_has_pinned_complete_pair() {
        let c = catalog().unwrap();
        assert_eq!(c.files.iter().map(|f| f.bytes).sum::<u64>(), 659959328);
        for f in c.files {
            assert_eq!(f.sha256.len(), 64);
            assert!(f.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
        }
    }
}
