//! Streaming SHA-256 through Windows CNG; no model is mapped into application RAM.
use std::{
    io::Read,
    path::Path,
    ptr::{null, null_mut},
};
use windows_sys::Win32::Security::Cryptography::*;

struct Algorithm(BCRYPT_ALG_HANDLE);
impl Drop for Algorithm {
    fn drop(&mut self) {
        unsafe {
            BCryptCloseAlgorithmProvider(self.0, 0);
        }
    }
}
struct Hash(BCRYPT_HASH_HANDLE);
impl Drop for Hash {
    fn drop(&mut self) {
        unsafe {
            BCryptDestroyHash(self.0);
        }
    }
}

pub fn sha256(mut source: impl Read) -> Result<String, String> {
    unsafe {
        let mut raw_algorithm = null_mut();
        if BCryptOpenAlgorithmProvider(&mut raw_algorithm, BCRYPT_SHA256_ALGORITHM, null(), 0) < 0 {
            return Err("Windows SHA-256 provider is unavailable.".into());
        }
        let algorithm = Algorithm(raw_algorithm);
        let mut raw_hash = null_mut();
        if BCryptCreateHash(algorithm.0, &mut raw_hash, null_mut(), 0, null(), 0, 0) < 0 {
            return Err("Cannot initialize SHA-256 verification.".into());
        }
        let hash = Hash(raw_hash);
        let mut buffer = [0u8; 65536];
        loop {
            let count = source
                .read(&mut buffer)
                .map_err(|_| "Cannot read file for integrity verification.")?;
            if count == 0 {
                break;
            }
            if BCryptHashData(hash.0, buffer.as_ptr(), count as u32, 0) < 0 {
                return Err("SHA-256 verification failed.".into());
            }
        }
        let mut digest = [0u8; 32];
        if BCryptFinishHash(hash.0, digest.as_mut_ptr(), 32, 0) < 0 {
            return Err("Cannot finalize SHA-256 verification.".into());
        }
        Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}
pub fn verify_file(path: &Path, expected_bytes: u64, expected_sha256: &str) -> Result<(), String> {
    let file = std::fs::File::open(path)
        .map_err(|_| "Required runtime/model file is missing or unreadable.")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect runtime/model file.")?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err("Runtime/model file size does not match its pinned manifest.".into());
    }
    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid SHA-256 manifest entry.".into());
    }
    if sha256(file)? != expected_sha256.to_ascii_lowercase() {
        return Err(
            "Runtime/model checksum mismatch. The file will not be executed or loaded.".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn standard_sha256_vector() {
        assert_eq!(
            sha256(&b"abc"[..]).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    #[test]
    fn rejects_corrupt_file_before_use() {
        let path =
            std::env::temp_dir().join(format!("voice-studio-hash-test-{}", std::process::id()));
        std::fs::write(&path, b"abc").unwrap();
        assert!(verify_file(&path, 3, &"0".repeat(64)).is_err());
        assert!(verify_file(&path, 2, &"0".repeat(64)).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
