use crate::resource::{self, ResourcePolicy};
use std::{
    ffi::OsStr,
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr::{null, null_mut},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{JobObjects::*, Memory::*, Threading::*},
};

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Windows command-line quoting following the CRT argument decoding convention.
fn quote(value: &OsStr) -> Vec<u16> {
    let mut result = vec![b'"' as u16];
    let mut slashes = 0;
    for ch in value.encode_wide() {
        if ch == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        if ch == b'"' as u16 {
            result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2 + 1));
        } else {
            result.extend(std::iter::repeat_n(b'\\' as u16, slashes));
        }
        slashes = 0;
        result.push(ch);
    }
    result.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    result.push(b'"' as u16);
    result
}

pub struct WorkerOutcome {
    pub exit_code: u32,
    pub peak_commit: u64,
}

#[derive(Default)]
pub struct LaunchContext<'a> {
    pub environment: &'a [(std::ffi::OsString, std::ffi::OsString)],
    pub directory: Option<&'a Path>,
    pub cleanup_confirmed: Option<&'a AtomicBool>,
    pub redirection: Option<crate::worker_io::Redirection<'a>>,
    pub cpu_rate_percent: Option<u32>,
}

pub fn run_admitted(
    executable: &Path,
    args: &[std::ffi::OsString],
    peak_ram: u64,
    peak_commit: u64,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
) -> Result<WorkerOutcome, String> {
    let mut lease = crate::scheduler::global().acquire()?;
    let memory = resource::snapshot()?;
    let policy = ResourcePolicy::conservative(&memory);
    policy.admit(&memory, peak_ram, peak_commit)?;
    let cleanup = AtomicBool::new(true);
    lease.require_cleanup();
    let result = run_owned_configured(
        executable,
        args,
        peak_commit,
        timeout,
        cancel,
        Some(&policy),
        LaunchContext {
            cleanup_confirmed: Some(&cleanup),
            ..Default::default()
        },
    );
    if cleanup.load(Ordering::Acquire) {
        lease.confirm_cleanup();
    }
    result
}

pub fn run_owned(
    executable: &Path,
    args: &[std::ffi::OsString],
    limit: u64,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    policy: Option<&ResourcePolicy>,
) -> Result<WorkerOutcome, String> {
    run_owned_configured(
        executable,
        args,
        limit,
        timeout,
        cancel,
        policy,
        LaunchContext::default(),
    )
}

pub fn run_owned_configured(
    executable: &Path,
    args: &[std::ffi::OsString],
    limit: u64,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    policy: Option<&ResourcePolicy>,
    context: LaunchContext<'_>,
) -> Result<WorkerOutcome, String> {
    if limit < 64 * 1024 * 1024
        || executable.as_os_str().encode_wide().any(|ch| ch == 0)
        || args.iter().any(|arg| arg.encode_wide().any(|ch| ch == 0))
    {
        return Err("Invalid worker limit or embedded NUL in worker arguments.".into());
    }
    if !executable.is_absolute() || !executable.is_file() {
        return Err("Choose an existing absolute executable path.".into());
    }
    let application: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut command = quote(executable.as_os_str());
    for arg in args {
        command.push(32);
        command.extend(quote(arg));
    }
    command.push(0);
    let directory_path = context
        .directory
        .unwrap_or_else(|| executable.parent().unwrap());
    if !directory_path.is_absolute() || !directory_path.is_dir() {
        return Err("Worker directory must be an existing absolute directory.".into());
    }
    let directory: Vec<u16> = directory_path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let environment = environment_block(context.environment)?;
    unsafe {
        let job = Handle(CreateJobObjectW(null(), null()));
        if job.0.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_JOB_MEMORY;
        limits.JobMemoryLimit =
            usize::try_from(limit).map_err(|_| "Worker limit is out of range")?;
        if SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as _,
            std::mem::size_of_val(&limits) as u32,
        ) == 0
        {
            return Err(format!(
                "Cannot enforce worker memory limit: {}",
                std::io::Error::last_os_error()
            ));
        }
        if let Some(percent) = context.cpu_rate_percent {
            if !(1..=100).contains(&percent) {
                return Err("Worker CPU rate must be between 1 and 100 percent.".into());
            }
            let cpu = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                    | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 {
                    CpuRate: percent * 100,
                },
            };
            if SetInformationJobObject(
                job.0,
                JobObjectCpuRateControlInformation,
                &cpu as *const _ as _,
                std::mem::size_of_val(&cpu) as u32,
            ) == 0
            {
                return Err(
                    "Cannot enforce the selected worker CPU limit; worker was not started.".into(),
                );
            }
        }
        let mut startup: STARTUPINFOEXW = std::mem::zeroed();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let io = context
            .redirection
            .map(crate::worker_io::StandardHandles::create)
            .transpose()?;
        if let Some(io) = &io {
            io.configure(&mut startup);
        }
        let mut process: PROCESS_INFORMATION = std::mem::zeroed();
        if CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            i32::from(io.is_some()),
            CREATE_SUSPENDED
                | CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT
                | if io.is_some() {
                    EXTENDED_STARTUPINFO_PRESENT
                } else {
                    0
                },
            environment.as_ptr() as _,
            directory.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        ) == 0
        {
            return Err(format!(
                "Cannot start worker: {}",
                std::io::Error::last_os_error()
            ));
        }
        let child = Handle(process.hProcess);
        let thread = Handle(process.hThread);
        if let Some(cleanup) = context.cleanup_confirmed {
            cleanup.store(false, Ordering::Release);
        }
        if AssignProcessToJobObject(job.0, child.0) == 0 {
            let error = std::io::Error::last_os_error();
            TerminateProcess(child.0, 1);
            if WaitForSingleObject(child.0, 5000) == WAIT_OBJECT_0
                && let Some(cleanup) = context.cleanup_confirmed
            {
                cleanup.store(true, Ordering::Release);
            }
            return Err(format!("Worker containment failed: {}", error));
        }
        if ResumeThread(thread.0) == u32::MAX {
            return Err("Could not resume contained worker.".into());
        }
        let started = Instant::now();
        let mut reason = None;
        loop {
            let wait = WaitForSingleObject(child.0, 200);
            if wait == WAIT_OBJECT_0 {
                break;
            }
            if wait == WAIT_FAILED {
                reason = Some("Worker wait failed.".to_string());
                break;
            }
            if let Some(redirection) = context.redirection
                && let Some(path) = redirection.log
                && std::fs::metadata(path)
                    .map(|m| m.len() > redirection.max_log_bytes)
                    .unwrap_or(true)
            {
                reason =
                    Some("Worker log exceeded its bounded budget or became unavailable.".into());
                break;
            }
            if cancel.load(Ordering::Acquire) {
                reason = Some("Cancelled; worker process tree stopped.".into());
                break;
            }
            if started.elapsed() > timeout {
                reason = Some("Worker timed out and was stopped. It was not retried.".into());
                break;
            }
            if let Some(policy) = policy {
                match resource::snapshot() {
                    Ok(m) if policy.critical(&m) => {
                        reason = Some(
                            "System memory pressure: worker stopped to preserve headroom.".into(),
                        );
                        break;
                    }
                    Err(_) => {
                        reason = Some(
                            "Resource monitoring failed; worker stopped conservatively.".into(),
                        );
                        break;
                    }
                    _ => {}
                }
            }
        }
        // Always drain descendants, even when the direct child exited normally.
        TerminateJobObject(job.0, 1);
        if WaitForSingleObject(child.0, 5000) != WAIT_OBJECT_0 {
            return Err(
                "Worker termination was not confirmed. Replacement work must remain blocked."
                    .into(),
            );
        }
        let drain_deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut active: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = std::mem::zeroed();
            if QueryInformationJobObject(
                job.0,
                JobObjectBasicAccountingInformation,
                &mut active as *mut _ as _,
                std::mem::size_of_val(&active) as u32,
                null_mut(),
            ) == 0
            {
                return Err(
                    "Cannot confirm worker subtree cleanup. Replacement work must remain blocked."
                        .into(),
                );
            }
            if active.ActiveProcesses == 0 {
                break;
            }
            if Instant::now() > drain_deadline {
                return Err(
                    "Worker descendants have not exited. Replacement work must remain blocked."
                        .into(),
                );
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if let Some(cleanup) = context.cleanup_confirmed {
            cleanup.store(true, Ordering::Release);
        }
        if let Some(redirection) = context.redirection
            && let Some(path) = redirection.log
        {
            match std::fs::metadata(path) {
                Ok(metadata) if metadata.len() > redirection.max_log_bytes => {
                    let truncated = std::fs::OpenOptions::new()
                        .write(true)
                        .open(path)
                        .and_then(|file| file.set_len(redirection.max_log_bytes));
                    reason = Some(
                        if truncated.is_ok() {
                            "Worker log exceeded its budget; excess output was discarded."
                        } else {
                            "Worker log exceeded its budget and could not be truncated."
                        }
                        .into(),
                    );
                }
                Err(_) => {
                    reason = Some("Worker log became unavailable.".into());
                }
                _ => {}
            }
        }
        if let Some(reason) = reason {
            return Err(reason);
        }
        let mut code = 0;
        if GetExitCodeProcess(child.0, &mut code) == 0 {
            return Err("Cannot read worker exit status.".into());
        }
        let mut accounting: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        if QueryInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            &mut accounting as *mut _ as _,
            std::mem::size_of_val(&accounting) as u32,
            null_mut(),
        ) == 0
        {
            return Err("Cannot read worker peak memory.".into());
        }
        Ok(WorkerOutcome {
            exit_code: code,
            peak_commit: accounting.PeakJobMemoryUsed as u64,
        })
    }
}

pub fn stdio_test() -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "Clock unavailable.")?
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("voice-studio-stdio-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).map_err(|_| "Cannot create I/O test directory.")?;
    let input = directory.join("input.txt");
    let log = directory.join("output.log");
    let result = (|| {
        let text = "Bounded native I/O — മലയാളം\n";
        std::fs::write(&input, text).map_err(|_| "Cannot write I/O fixture.")?;
        let outcome = run_owned_configured(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &["--stdio-probe".into()],
            128 * 1024 * 1024,
            Duration::from_secs(15),
            Arc::new(AtomicBool::new(false)),
            None,
            LaunchContext {
                cpu_rate_percent: Some(25),
                redirection: Some(crate::worker_io::Redirection {
                    input: Some(&input),
                    log: Some(&log),
                    max_log_bytes: 16384,
                }),
                ..Default::default()
            },
        )?;
        if outcome.exit_code != 0 {
            return Err(format!("I/O fixture exited {}.", outcome.exit_code));
        }
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&log).map_err(|_| "Cannot read I/O fixture result.")?,
        )
        .map_err(|_| "I/O fixture returned invalid JSON.")?;
        if value["text"] != text {
            return Err("Worker input encoding changed.".into());
        }
        println!(
            "PASS: Unicode stdin and bounded stdout passed through an explicit inherited-handle whitelist; worker subtree drained."
        );
        let spam_log = directory.join("spam.log");
        let spam = run_owned_configured(
            &std::env::current_exe().map_err(|e| e.to_string())?,
            &["--log-spam-probe".into()],
            128 * 1024 * 1024,
            Duration::from_secs(15),
            Arc::new(AtomicBool::new(false)),
            None,
            LaunchContext {
                redirection: Some(crate::worker_io::Redirection {
                    input: None,
                    log: Some(&spam_log),
                    max_log_bytes: 16384,
                }),
                ..Default::default()
            },
        );
        if !matches!(&spam,Err(error) if error.contains("log exceeded"))
            || std::fs::metadata(&spam_log)
                .map(|m| m.len() > 16384)
                .unwrap_or(true)
        {
            return Err(
                "Oversized worker output did not fail within its bounded log contract.".into(),
            );
        }
        let _ = std::fs::remove_file(spam_log);
        println!(
            "PASS: an oversized worker log failed the job and was truncated to its 16 KiB diagnostic budget."
        );
        Ok(())
    })();
    let _ = std::fs::remove_file(input);
    let _ = std::fs::remove_file(log);
    let _ = std::fs::remove_dir(directory);
    result
}

fn environment_block(
    overrides: &[(std::ffi::OsString, std::ffi::OsString)],
) -> Result<Vec<u16>, String> {
    let mut values = std::collections::BTreeMap::new();
    for (name, value) in std::env::vars_os() {
        values.insert(name.to_string_lossy().to_uppercase(), (name, value));
    }
    for (name, value) in overrides {
        if name.is_empty()
            || name.encode_wide().any(|ch| ch == 0 || ch == b'=' as u16)
            || value.encode_wide().any(|ch| ch == 0)
        {
            return Err("Invalid worker environment override.".into());
        }
        values.insert(
            name.to_string_lossy().to_uppercase(),
            (name.clone(), value.clone()),
        );
    }
    let mut block = Vec::new();
    for (_, (name, value)) in values {
        block.extend(name.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

pub fn allocation_probe() {
    unsafe {
        let first = VirtualAlloc(
            null(),
            16 * 1024 * 1024,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        );
        if first.is_null() {
            std::process::exit(2);
        }
        let excessive = VirtualAlloc(
            null(),
            512 * 1024 * 1024,
            MEM_RESERVE | MEM_COMMIT,
            PAGE_READWRITE,
        );
        if !excessive.is_null() {
            VirtualFree(excessive, 0, MEM_RELEASE);
            std::process::exit(3);
        }
        VirtualFree(first, 0, MEM_RELEASE);
    }
}

pub fn containment_test() -> Result<(), String> {
    let result = run_owned(
        &std::env::current_exe().map_err(|e| e.to_string())?,
        &["--allocation-probe".into()],
        128 * 1024 * 1024,
        Duration::from_secs(10),
        Arc::new(AtomicBool::new(false)),
        None,
    )?;
    if result.exit_code != 0 {
        return Err(format!(
            "Allocation containment test failed: {}",
            result.exit_code
        ));
    }
    println!(
        "PASS: small allocation succeeded; 512 MiB commit refused inside 128 MiB job. Peak worker commit: {} bytes. Parent survived.",
        result.peak_commit
    );
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let timeout = run_owned(
        &executable,
        &["--sleep-probe".into()],
        128 * 1024 * 1024,
        Duration::from_millis(500),
        Arc::new(AtomicBool::new(false)),
        None,
    );
    if !matches!(&timeout,Err(reason) if reason.contains("timed out")) {
        return Err("Worker timeout probe did not stop correctly.".into());
    }
    let cancellation = run_owned(
        &executable,
        &["--sleep-probe".into()],
        128 * 1024 * 1024,
        Duration::from_secs(10),
        Arc::new(AtomicBool::new(true)),
        None,
    );
    if !matches!(&cancellation,Err(reason) if reason.contains("Cancelled")) {
        return Err("Worker cancellation probe did not stop correctly.".into());
    }
    println!(
        "PASS: timeout and cancellation terminated and drained owned Jobs. No replacement work started before cleanup confirmation."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_overrides_are_case_insensitive_and_unicode_safe() {
        let block = environment_block(&[
            ("Path".into(), "C:\\native-only".into()),
            ("VOICE_STUDIO_TEST".into(), "മലയാളം".into()),
        ])
        .unwrap();
        assert!(block.ends_with(&[0, 0]));
        let text = String::from_utf16(&block).unwrap();
        let path_entries: Vec<_> = text
            .split('\0')
            .filter(|part| part.to_uppercase().starts_with("PATH="))
            .collect();
        assert_eq!(path_entries.len(), 1);
        assert_eq!(path_entries[0], "Path=C:\\native-only");
        assert!(text.contains("VOICE_STUDIO_TEST=മലയാളം"));
        assert!(environment_block(&[("INVALID=KEY".into(), "value".into())]).is_err());
        assert!(environment_block(&[("KEY".into(), "embedded\0nul".into())]).is_err());
    }
    #[test]
    fn arguments_preserve_quotes_and_trailing_slashes() {
        let value = String::from_utf16(&quote(OsStr::new("C:\\some path\\"))).unwrap();
        assert_eq!(value, "\"C:\\some path\\\\\"");
        assert_eq!(
            String::from_utf16(&quote(OsStr::new("a\"b"))).unwrap(),
            "\"a\\\"b\""
        );
    }
}
