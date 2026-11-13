#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
#[cfg(windows)]
mod backend;
mod resource;
#[cfg(windows)]
mod runtime;
mod scheduler;
mod state;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod worker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
