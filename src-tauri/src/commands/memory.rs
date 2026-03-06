use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Serialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
    /// RSS (resident set size) of this process in bytes.
    pub process_used: u64,
}

#[tauri::command]
pub fn get_memory_info() -> Result<MemoryInfo, String> {
    let pid = Pid::from_u32(std::process::id());

    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);

    let process_used = sys
        .process(pid)
        .map(|p| p.memory())
        .unwrap_or(0);

    Ok(MemoryInfo {
        total: sys.total_memory(),
        available: sys.available_memory(),
        used: sys.used_memory(),
        process_used,
    })
}
