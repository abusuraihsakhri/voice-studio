use crate::{
    resource,
    worker::{self, LaunchContext},
};
use std::{
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn check(python: &Path) -> Result<serde_json::Value, String> {
    if resource::snapshot()?.available_ram < 256 * 1024 * 1024 {
        return Err("Insufficient headroom even for the bounded runtime preflight.".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock unavailable.")?
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "voice-studio-runtime-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&directory).map_err(|_| "Cannot create an isolated runtime-check directory.")?;
    let report_path = directory.join("report.json");
    let result = (|| {
        let overrides = vec![
            ("OMNIVOICE_DEVICE".into(), "cpu".into()),
            ("OMNIVOICE_SKIP_GPU_PROBE".into(), "1".into()),
            ("HF_HUB_OFFLINE".into(), "1".into()),
            ("TRANSFORMERS_OFFLINE".into(), "1".into()),
        ];
        let args = vec![
            "-I".into(),
            "-c".into(),
            include_str!("../python/runtime_probe.py").into(),
            report_path.as_os_str().to_owned(),
        ];
        let outcome = worker::run_owned_configured(
            python,
            &args,
            128 * 1024 * 1024,
            Duration::from_secs(15),
            Arc::new(AtomicBool::new(false)),
            None,
            LaunchContext {
                environment: &overrides,
                directory: Some(&directory),
                cleanup_confirmed: None,
                redirection: None,
                cpu_rate_percent: None,
            },
        )?;
        if outcome.exit_code != 0 {
            return Err(format!(
                "Selected Python runtime probe exited {}. No packages were installed or repaired.",
                outcome.exit_code
            ));
        }
        let file = fs::File::open(&report_path)
            .map_err(|_| "Runtime did not produce its preflight report.")?;
        let mut bytes = Vec::new();
        file.take(32769)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read runtime preflight report.")?;
        if bytes.len() > 32768 {
            return Err("Runtime report exceeded its bounded size.".into());
        }
        let report: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid runtime preflight report.")?;
        if report["schema_version"] != 1
            || report["scope"] != "module-discovery-only"
            || !report["ready"].is_boolean()
            || report["gpu_modules_imported"] != false
            || report["device_policy"] != "cpu"
        {
            return Err("Runtime report violated the CPU-only discovery contract.".into());
        }
        Ok(report)
    })();
    // Only these two freshly created paths are removed; never recurse over user data.
    let _ = fs::remove_file(report_path);
    let _ = fs::remove_dir(directory);
    result
}
