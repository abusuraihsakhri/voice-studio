#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
#[cfg(windows)]
mod backend;
mod resource;
mod state;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod worker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
