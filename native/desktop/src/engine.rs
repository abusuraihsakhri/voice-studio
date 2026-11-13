use crate::{
    integrity, resource,
    worker::{self, LaunchContext},
    worker_io::Redirection,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SOURCE_COMMIT: &str = "040c8b344d8c670ce1475194751d119b5ef82c78";
pub const GGML_COMMIT: &str = "d5152bf4348453068aa8de0d479caef40a613b9d";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifest {
    pub schema_version: u32,
    pub engine: String,
    pub executable: String,
    pub sha256: String,
    pub bytes: u64,
    pub source_commit: String,
    pub ggml_commit: String,
    pub build_options: BTreeMap<String, bool>,
}
impl RuntimeManifest {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.engine != "omnivoice-gguf-cpu"
            || self.executable != "omnivoice-tts.exe"
            || self.source_commit != SOURCE_COMMIT
            || self.ggml_commit != GGML_COMMIT
            || self.bytes == 0
        {
            return Err("Unsupported native runtime manifest.".into());
        }
        for name in [
            "GGML_CUDA",
            "GGML_VULKAN",
            "GGML_METAL",
            "GGML_SYCL",
            "GGML_BACKEND_DL",
            "BUILD_SHARED_LIBS",
        ] {
            if self.build_options.get(name) != Some(&false) {
                return Err("Native conservative runtime must be a static CPU-only build.".into());
            }
        }
        if self.build_options.get("GGML_CPU") != Some(&true) {
            return Err("Native CPU backend is absent from its manifest.".into());
        }
        Ok(())
    }
}
#[derive(Serialize)]
pub struct EngineProbe {
    pub engine: String,
    pub runtime_checksum_verified: bool,
    pub cpu_only_build: bool,
    pub cli_contract_verified: bool,
    pub model_execution_verified: bool,
}

pub fn verified_manifest(directory: &Path) -> Result<RuntimeManifest, String> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("Select an existing absolute native runtime directory.".into());
    }
    let file = fs::File::open(directory.join("engine.json"))
        .map_err(|_| "Native runtime manifest is missing.")?;
    let mut bytes = Vec::new();
    file.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read native runtime manifest.")?;
    if bytes.len() > 32768 {
        return Err("Native runtime manifest is too large.".into());
    }
    let manifest: RuntimeManifest =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid native runtime manifest.")?;
    manifest.validate()?;
    let executable = directory.join(&manifest.executable);
    integrity::verify_file(&executable, manifest.bytes, &manifest.sha256)?;
    Ok(manifest)
}
pub fn probe(directory: &Path) -> Result<EngineProbe, String> {
    if resource::snapshot()?.available_ram < 256 * 1024 * 1024 {
        return Err("Insufficient headroom for the bounded native-engine preflight.".into());
    }
    let manifest = verified_manifest(directory)?;
    let executable = directory.join(&manifest.executable);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Clock unavailable.")?
        .as_nanos();
    let temporary = std::env::temp_dir().join(format!(
        "voice-studio-engine-probe-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&temporary).map_err(|_| "Cannot create native engine preflight directory.")?;
    let log = temporary.join("help.log");
    let result = (|| {
        let env = vec![("GGML_BACKEND".into(), "CPU".into())];
        let outcome = worker::run_owned_configured(
            &executable,
            &["--help".into()],
            128 * 1024 * 1024,
            Duration::from_secs(20),
            Arc::new(AtomicBool::new(false)),
            None,
            LaunchContext {
                environment: &env,
                directory: Some(directory),
                redirection: Some(Redirection {
                    input: None,
                    log: Some(&log),
                    max_log_bytes: 32768,
                }),
                ..Default::default()
            },
        )?;
        if outcome.exit_code != 0 {
            return Err(format!(
                "Native runtime preflight exited {}. No models were loaded.",
                outcome.exit_code
            ));
        }
        let help =
            fs::read_to_string(&log).map_err(|_| "Cannot read native runtime preflight result.")?;
        if !help.contains("--model")
            || !help.contains("--codec")
            || !help.contains("--ref-wav")
            || !help.contains("stdin")
        {
            return Err("Native runtime CLI does not match the pinned inference contract.".into());
        }
        Ok(EngineProbe {
            engine: manifest.engine,
            runtime_checksum_verified: true,
            cpu_only_build: true,
            cli_contract_verified: true,
            model_execution_verified: false,
        })
    })();
    let _ = fs::remove_file(log);
    let _ = fs::remove_dir(temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unpinned_and_gpu_runtime_manifests() {
        let options = [
            ("GGML_CUDA", false),
            ("GGML_VULKAN", false),
            ("GGML_METAL", false),
            ("GGML_SYCL", false),
            ("GGML_BACKEND_DL", false),
            ("BUILD_SHARED_LIBS", false),
            ("GGML_CPU", true),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let mut m = RuntimeManifest {
            schema_version: 1,
            engine: "omnivoice-gguf-cpu".into(),
            executable: "omnivoice-tts.exe".into(),
            sha256: "0".repeat(64),
            bytes: 100,
            source_commit: SOURCE_COMMIT.into(),
            ggml_commit: GGML_COMMIT.into(),
            build_options: options,
        };
        assert!(m.validate().is_ok());
        m.build_options.insert("GGML_CUDA".into(), true);
        assert!(m.validate().is_err());
        m.build_options.insert("GGML_CUDA".into(), false);
        m.executable = "../other.exe".into();
        assert!(m.validate().is_err());
    }
}
