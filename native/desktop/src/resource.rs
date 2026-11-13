use serde::{Deserialize, Serialize};

pub const GIB: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub total_ram: u64,
    pub available_ram: u64,
    pub available_commit: u64,
    pub process_private_commit: u64,
    pub process_working_set: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePolicy {
    pub ram_reserve: u64,
    pub commit_reserve: u64,
    pub worker_commit_limit: u64,
}

impl ResourcePolicy {
    pub fn conservative(memory: &MemorySnapshot) -> Self {
        Self {
            ram_reserve: (memory.total_ram / 4).max(2 * GIB),
            commit_reserve: 2 * GIB,
            worker_commit_limit: (memory.total_ram / 3).min(6 * GIB),
        }
    }

    pub fn admit(
        &self,
        memory: &MemorySnapshot,
        peak_ram: u64,
        peak_commit: u64,
    ) -> Result<(), String> {
        if self.ram_reserve < GIB
            || self.commit_reserve < GIB
            || self.worker_commit_limit < 256 * 1024 * 1024
        {
            return Err(
                "Invalid resource policy; safe reserves and worker limits are required.".into(),
            );
        }
        if peak_ram == 0 || peak_commit == 0 {
            return Err("This engine has no validated memory envelope. Generation is unavailable in conservative mode.".into());
        }
        if peak_ram > memory.available_ram.saturating_sub(self.ram_reserve) {
            return Err("Not enough available RAM while preserving system headroom. Close other applications or use a smaller engine.".into());
        }
        if peak_commit > self.worker_commit_limit
            || peak_commit > memory.available_commit.saturating_sub(self.commit_reserve)
        {
            return Err(
                "The job exceeds the worker or system commit budget. Use a smaller engine.".into(),
            );
        }
        Ok(())
    }

    pub fn critical(&self, memory: &MemorySnapshot) -> bool {
        memory.available_ram < self.ram_reserve / 2
            || memory.available_commit < self.commit_reserve / 2
    }
}

#[cfg(windows)]
pub fn snapshot() -> Result<MemorySnapshot, String> {
    use windows_sys::Win32::System::{
        ProcessStatus::*, SystemInformation::*, Threading::GetCurrentProcess,
    };
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of_val(&status) as u32;
        if GlobalMemoryStatusEx(&mut status) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let mut performance: PERFORMANCE_INFORMATION = std::mem::zeroed();
        performance.cb = std::mem::size_of_val(&performance) as u32;
        if GetPerformanceInfo(&mut performance, std::mem::size_of_val(&performance) as u32) == 0 {
            return Err(
                "System commit telemetry unavailable; no inference will be admitted.".into(),
            );
        }
        let mut process: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        process.cb = std::mem::size_of_val(&process) as u32;
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut process as *mut _ as *mut _,
            process.cb,
        ) == 0
        {
            return Err("Process memory telemetry unavailable.".into());
        }
        Ok(MemorySnapshot {
            total_ram: status.ullTotalPhys,
            available_ram: status.ullAvailPhys,
            available_commit: (performance
                .CommitLimit
                .saturating_sub(performance.CommitTotal) as u64)
                .saturating_mul(performance.PageSize as u64),
            process_private_commit: process.PrivateUsage as u64,
            process_working_set: process.WorkingSetSize as u64,
        })
    }
}

#[cfg(not(windows))]
pub fn snapshot() -> Result<MemorySnapshot, String> {
    Err("Windows telemetry is not available on this platform.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn memory() -> MemorySnapshot {
        MemorySnapshot {
            total_ram: 8 * GIB,
            available_ram: 5 * GIB,
            available_commit: 7 * GIB,
            process_private_commit: 0,
            process_working_set: 0,
        }
    }
    #[test]
    fn reserves_host_capacity() {
        let m = memory();
        let p = ResourcePolicy::conservative(&m);
        assert_eq!(p.ram_reserve, 2 * GIB);
        assert!(p.admit(&m, 2 * GIB, 2 * GIB).is_ok());
        assert!(p.admit(&m, 4 * GIB, 2 * GIB).is_err());
    }
    #[test]
    fn independent_commit_check() {
        let mut m = memory();
        let p = ResourcePolicy::conservative(&m);
        m.available_commit = GIB;
        assert!(p.admit(&m, GIB, GIB).is_err());
    }
    #[test]
    fn refuses_unknown_envelope_and_underflow() {
        let mut m = memory();
        let p = ResourcePolicy::conservative(&m);
        assert!(p.admit(&m, 0, GIB).is_err());
        m.available_ram = 1;
        assert!(p.admit(&m, GIB, GIB).is_err());
        assert!(p.critical(&m));
    }
}
