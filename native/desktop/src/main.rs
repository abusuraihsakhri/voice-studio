#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod generation;
#[cfg(windows)]
mod integrity;
#[cfg(windows)]
mod models;
mod provenance;
mod resource;
#[cfg(windows)]
mod runtime;
mod scheduler;
mod state;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod worker;
#[cfg(windows)]
mod worker_io;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--log-spam-probe") {
        use std::io::Write;
        let block = [b'x'; 65536];
        for _ in 0..8 {
            if std::io::stdout().write_all(&block).is_err() {
                break;
            }
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--profile-native-cpu") {
        if args.len() != 5 {
            eprintln!(
                "Usage: --profile-native-cpu RUNTIME_DIRECTORY MODEL_DIRECTORY NEW_WAV_OUTPUT"
            );
            std::process::exit(1);
        }
        match generation::profile_short(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            std::path::Path::new(&args[4]),
        ) {
            Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
            Err(error) => {
                eprintln!("Native synthesis fixture failed or was refused: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--model-check") {
        let result = args
            .get(2)
            .ok_or_else(|| "Usage: --model-check ABSOLUTE_MODEL_DIRECTORY".to_string())
            .and_then(|path| models::verify(std::path::Path::new(path)));
        match result {
            Ok(catalog) => println!(
                "{}",
                serde_json::json!({"model_revision":catalog.revision,"verified_files":catalog.files.len(),"model_execution_verified":false})
            ),
            Err(error) => {
                eprintln!("Model integrity check failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--engine-probe") {
        let result = args
            .get(2)
            .ok_or_else(|| "Usage: --engine-probe ABSOLUTE_RUNTIME_DIRECTORY".to_string())
            .and_then(|path| engine::probe(std::path::Path::new(path)));
        match result {
            Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
            Err(error) => {
                eprintln!("Native engine probe failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if args
        .get(1)
        .is_some_and(|arg| matches!(arg.as_str(), "--run-tool" | "--run-tool-log"))
    {
        let result = (|| {
            let limit = args
                .get(2)
                .and_then(|v| v.parse::<u64>().ok())
                .and_then(|v| v.checked_mul(1024 * 1024))
                .ok_or(
                    "Usage: --run-tool COMMIT_MIB TIMEOUT_SECONDS ABSOLUTE_EXECUTABLE [ARGS...]",
                )?;
            let timeout = args
                .get(3)
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| (1..=3600).contains(v))
                .ok_or("Tool timeout must be between 1 and 3600 seconds.")?;
            let logged = args[1] == "--run-tool-log";
            let program_index = if logged { 5 } else { 4 };
            let program = args
                .get(program_index)
                .ok_or("An explicit absolute build-tool executable is required.")?;
            let memory = resource::snapshot()?;
            // Build tools have a separate conservative profile; model admission is unchanged.
            let policy = resource::ResourcePolicy {
                ram_reserve: (memory.total_ram / 8).max(2 * resource::GIB),
                commit_reserve: 2 * resource::GIB,
                worker_commit_limit: (memory.total_ram / 3).min(6 * resource::GIB),
            };
            policy.admit(&memory, limit, limit)?;
            let tool_args = args[program_index + 1..]
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>();
            worker::run_owned_configured(
                std::path::Path::new(program),
                &tool_args,
                limit,
                std::time::Duration::from_secs(timeout),
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                Some(&policy),
                worker::LaunchContext {
                    redirection: if logged {
                        Some(worker_io::Redirection {
                            input: None,
                            log: Some(std::path::Path::new(&args[4])),
                            max_log_bytes: 2 * 1024 * 1024,
                        })
                    } else {
                        None
                    },
                    ..Default::default()
                },
            )
        })();
        match result {
            Ok(outcome) => {
                println!(
                    "Bounded tool exit: {}; Windows Job peak commit accounting: {} bytes.",
                    outcome.exit_code, outcome.peak_commit
                );
                if outcome.exit_code != 0 {
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("Bounded tool refused or stopped: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--stdio-probe") {
        use std::io::Read;
        let mut text = String::new();
        if std::io::stdin()
            .take(128 * 1024 + 1)
            .read_to_string(&mut text)
            .is_err()
            || text.len() > 128 * 1024
        {
            std::process::exit(2);
        }
        println!("{}", serde_json::json!({"text":text}));
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--stdio-test") {
        if let Err(error) = worker::stdio_test() {
            eprintln!("Standard I/O test failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--runtime-check") {
        let result = args
            .get(2)
            .ok_or_else(|| "Usage: --runtime-check ABSOLUTE_PYTHON_EXE".to_string())
            .and_then(|path| runtime::check(std::path::Path::new(path)));
        match result {
            Ok(report) => {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                if report["ready"] != true {
                    std::process::exit(2);
                }
            }
            Err(error) => {
                eprintln!("Runtime check failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--admitted-probe") {
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|exe| {
                worker::run_admitted(
                    &exe,
                    &["--allocation-probe".into()],
                    128 * 1024 * 1024,
                    128 * 1024 * 1024,
                    std::time::Duration::from_secs(10),
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                )
            });
        match result {
            Ok(outcome) if outcome.exit_code == 0 => println!(
                "PASS: admitted one bounded worker after fresh host checks; oversized commit was refused and cleanup confirmed."
            ),
            Ok(outcome) => {
                eprintln!("Admitted probe exited {}", outcome.exit_code);
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("Admitted probe refused or stopped: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).is_some_and(|arg| arg == "--check-budget") {
        let result = (|| {
            let ram = args
                .get(2)
                .and_then(|v| v.parse::<u64>().ok())
                .and_then(|v| v.checked_mul(1024 * 1024))
                .ok_or("Usage: --check-budget PEAK_RAM_MIB PEAK_COMMIT_MIB")?;
            let commit = args
                .get(3)
                .and_then(|v| v.parse::<u64>().ok())
                .and_then(|v| v.checked_mul(1024 * 1024))
                .ok_or("Usage: --check-budget PEAK_RAM_MIB PEAK_COMMIT_MIB")?;
            let memory = resource::snapshot()?;
            resource::ResourcePolicy::conservative(&memory).admit(&memory, ram, commit)?;
            Ok::<(), String>(())
        })();
        match result {
            Ok(()) => println!(
                "Budget fits current reserves. This check does not certify an engine's memory envelope."
            ),
            Err(e) => {
                eprintln!("Budget refused: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--diagnostics") {
        match resource::snapshot() {
            Ok(memory) => println!("{}", serde_json::to_string_pretty(&memory).unwrap()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--containment-test") {
        if let Err(error) = worker::containment_test() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(windows)]
    if args.get(1).is_some_and(|arg| arg == "--sleep-probe") {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return;
    }
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--allocation-probe") {
        // Disposable test child: a denied commit must not affect its parent.
        worker::allocation_probe();
        return;
    }
    #[cfg(windows)]
    if let Err(error) = ui::run() {
        let message = ui::wide(&error);
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                message.as_ptr(),
                ui::wide("Voice Studio").as_ptr(),
                0x10,
            );
        }
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    eprintln!("The native desktop currently targets Windows. Resource-policy tests are portable.");
}
