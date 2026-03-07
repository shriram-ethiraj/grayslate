use serde::Serialize;
use sysinfo::System;

#[derive(Serialize)]
pub struct MemoryInfo {
    pub available: u64,
    pub used: u64,
}

#[tauri::command]
pub fn get_memory_info() -> Result<MemoryInfo, String> {
    let mut sys = System::new();
    sys.refresh_memory();

    Ok(MemoryInfo {
        available: sys.available_memory(),
        used: sys.used_memory(),
    })
}
