//! Bounded engineering fixture for establishing the first native CPU memory profile.
//! This is not an unrestricted production generation endpoint.
use crate::{
    engine, integrity, models, provenance,
    resource::{self, GIB, ResourcePolicy},
    scheduler,
    worker::{self, LaunchContext},
    worker_io::Redirection,
};
use std::{
    fs,
    io::Write,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const FIXTURE_TEXT: &str = "This is a native CPU voice test.";

fn unique_workspace(parent: &Path) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Clock unavailable.")?
        .as_nanos();
    let path = parent.join(format!(".native-take-{}-{nonce}", std::process::id()));
    fs::create_dir(&path).map_err(|_| "Cannot create isolated take directory.")?;
    Ok(path)
}

pub fn profile_short(
    runtime: &Path,
    model_directory: &Path,
    output: &Path,
) -> Result<serde_json::Value, String> {
    if !output.is_absolute()
        || output
            .extension()
            .is_none_or(|v| !v.eq_ignore_ascii_case("wav"))
    {
        return Err("Select an absolute, new WAV output path for the profiling fixture.".into());
    }
    let parent = output.parent().ok_or("Output directory is missing.")?;
    if !parent.is_dir() || output.exists() {
        return Err(
            "The output parent must exist and the selected WAV must not already exist.".into(),
        );
    }
    let report_path = output.with_extension("profile.json");
    if report_path.exists() {
        return Err("The profiling report already exists and will not be overwritten.".into());
    }
    let mut lease = scheduler::global().acquire()?;
    let manifest = engine::verified_manifest(runtime)?;
    let catalog = models::verify(model_directory)?;
    let snapshot = resource::snapshot()?;
    // Engineering-only fixed 3-second fixture: hard allocation ceiling, CPU cap,
    // and fresh reserve check. This does not certify arbitrary text/reference jobs.
    let cap = GIB;
    let policy = ResourcePolicy {
        ram_reserve: (snapshot.total_ram / 8).max(2 * GIB),
        commit_reserve: 2 * GIB,
        worker_commit_limit: cap,
    };
    policy.admit(&snapshot, cap + 64 * 1024 * 1024, cap)?;
    let workspace = unique_workspace(parent)?;
    let input = workspace.join("input.txt");
    let raw = workspace.join("raw.wav");
    let marked = workspace.join("marked.wav");
    let log = workspace.join("engine.log");
    let staged_report = workspace.join("profile.json.partial");
    let started = Instant::now();
    let result = (|| {
        fs::write(&input, FIXTURE_TEXT).map_err(|_| "Cannot stage native synthesis text.")?;
        let args = vec![
            "--model".into(),
            model_directory
                .join(&catalog.files[0].name)
                .into_os_string(),
            "--codec".into(),
            model_directory
                .join(&catalog.files[1].name)
                .into_os_string(),
            "--duration".into(),
            "3".into(),
            "--seed".into(),
            "42".into(),
            "--steps".into(),
            "32".into(),
            "-o".into(),
            raw.as_os_str().to_owned(),
        ];
        let env = vec![
            ("GGML_BACKEND".into(), "CPU".into()),
            ("OMP_NUM_THREADS".into(), "2".into()),
            ("HF_HUB_OFFLINE".into(), "1".into()),
        ];
        let cleanup = AtomicBool::new(true);
        lease.require_cleanup();
        let outcome = worker::run_owned_configured(
            &runtime.join(&manifest.executable),
            &args,
            cap,
            Duration::from_secs(900),
            Arc::new(AtomicBool::new(false)),
            Some(&policy),
            LaunchContext {
                environment: &env,
                directory: Some(&workspace),
                cleanup_confirmed: Some(&cleanup),
                redirection: Some(Redirection {
                    input: Some(&input),
                    log: Some(&log),
                    max_log_bytes: 2 * 1024 * 1024,
                }),
                cpu_rate_percent: Some(25),
            },
        );
        if cleanup.load(Ordering::Acquire) {
            lease.confirm_cleanup();
        }
        let outcome = outcome?;
        if outcome.exit_code != 0 {
            return Err(format!(
                "Native CPU fixture exited {}. No completed take was published; bounded diagnostics remain in its take directory.",
                outcome.exit_code
            ));
        }
        let metadata = serde_json::json!({"synthetic":true,"engine":manifest.engine,"runtime_sha256":manifest.sha256,"model_revision":catalog.revision,"seed":42,"marking":"RIFF-metadata-only; not an AudioSeal watermark"});
        let info = provenance::mark_synthetic(&raw, &marked, &metadata)?;
        let marked_hash = integrity::sha256(
            fs::File::open(&marked).map_err(|_| "Cannot verify completed take.")?,
        )?;
        let report = serde_json::json!({"schema_version":1,"scope":"fixed-short-cpu-fixture","text":FIXTURE_TEXT,"seed":42,"steps":32,"requested_duration_seconds":3,"audio":info,"audio_sha256":marked_hash,"runtime_sha256":manifest.sha256,"model_revision":catalog.revision,"job_commit_limit_bytes":cap,"peak_job_commit_accounting_bytes":outcome.peak_commit,"cpu_rate_percent":25,"elapsed_seconds":started.elapsed().as_secs_f64(),"general_model_memory_profile_certified":false,"speech_content_and_voice_quality_reviewed":false});
        let mut report_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged_report)
            .map_err(|_| "Cannot create profiling report without overwriting an existing file.")?;
        report_file
            .write_all(
                &serde_json::to_vec_pretty(&report)
                    .map_err(|_| "Cannot serialize profiling report.")?,
            )
            .map_err(|_| "Cannot write profiling report.")?;
        report_file
            .sync_all()
            .map_err(|_| "Cannot flush profiling report.")?;
        drop(report_file);
        let from: Vec<u16> = marked.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = output.as_os_str().encode_wide().chain(Some(0)).collect();
        // Windows MoveFileW does not replace an existing destination; staging is
        // in the same output parent so publishing is an atomic same-volume move.
        if unsafe { windows_sys::Win32::Storage::FileSystem::MoveFileW(from.as_ptr(), to.as_ptr()) }
            == 0
        {
            return Err("Cannot publish the take without overwriting a destination. Staged audio/report were preserved.".into());
        }
        let from_report: Vec<u16> = staged_report
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let to_report: Vec<u16> = report_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileW(
                from_report.as_ptr(),
                to_report.as_ptr(),
            )
        } == 0
        {
            return Err("The validated WAV was published, but its report destination was unavailable. The staged report was preserved for recovery.".into());
        }
        Ok(report)
    })();
    if result.is_ok() {
        for file in [input, raw, log] {
            let _ = fs::remove_file(file);
        }
        let _ = fs::remove_dir(workspace);
    }
    // Failed runs retain their bounded local diagnostics and input for review.
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_overwrites_an_existing_take() {
        let path = std::env::temp_dir().join(format!(
            "voice-studio-existing-take-{}.wav",
            std::process::id()
        ));
        fs::write(&path, b"original-user-file").unwrap();
        let result = profile_short(
            Path::new("C:\\missing-runtime"),
            Path::new("C:\\missing-models"),
            &path,
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original-user-file");
        fs::remove_file(path).unwrap();
    }
}
