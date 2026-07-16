use serde::Serialize;

#[derive(Serialize)]
pub struct MemoryInfo {
    pub available: u64,
    pub used: u64,
}

#[tauri::command]
pub fn get_memory_info() -> Result<MemoryInfo, String> {
    let (available, used) = platform_memory()?;
    Ok(MemoryInfo { available, used })
}

// ── Windows ─────────────────────────────────────────────────────────
#[cfg(target_os = "windows")]
fn platform_memory() -> Result<(u64, u64), String> {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
    }
    unsafe {
        let mut ms = std::mem::zeroed::<MemoryStatusEx>();
        ms.dw_length = std::mem::size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(&mut ms) == 0 {
            return Err("GlobalMemoryStatusEx failed".into());
        }
        Ok((
            ms.ull_avail_phys,
            ms.ull_total_phys.saturating_sub(ms.ull_avail_phys),
        ))
    }
}

// ── macOS ───────────────────────────────────────────────────────────
#[cfg(target_os = "macos")]
fn platform_memory() -> Result<(u64, u64), String> {
    // Total physical memory via sysctl HW_MEMSIZE.
    fn total_memory() -> Result<u64, String> {
        // CTL_HW = 6, HW_MEMSIZE = 24
        let mut mib: [i32; 2] = [6, 24];
        let mut value: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        extern "C" {
            fn sysctl(
                name: *mut i32,
                namelen: u32,
                oldp: *mut u8,
                oldlenp: *mut usize,
                newp: *const u8,
                newlen: usize,
            ) -> i32;
        }
        let ret = unsafe {
            sysctl(
                mib.as_mut_ptr(),
                2,
                &mut value as *mut u64 as *mut u8,
                &mut size,
                std::ptr::null(),
                0,
            )
        };
        if ret != 0 {
            return Err("sysctl HW_MEMSIZE failed".into());
        }
        Ok(value)
    }

    // Available memory via host_statistics64 (vm_statistics64).
    fn available_memory() -> Result<u64, String> {
        #[repr(C)]
        #[allow(non_camel_case_types)]
        struct vm_statistics64 {
            free_count: u32,
            active_count: u32,
            inactive_count: u32,
            wire_count: u32,
            zero_fill_count: u64,
            reactivations: u64,
            pageins: u64,
            pageouts: u64,
            faults: u64,
            cow_faults: u64,
            lookups: u64,
            hits: u64,
            purges: u64,
            purgeable_count: u32,
            speculative_count: u32,
            decompressions: u64,
            compressions: u64,
            swapins: u64,
            swapouts: u64,
            compressor_page_count: u32,
            throttled_count: u32,
            external_page_count: u32,
            internal_page_count: u32,
            total_uncompressed_pages_in_compressor: u64,
        }

        // HOST_VM_INFO64 = 4, KERN_SUCCESS = 0
        const HOST_VM_INFO64: i32 = 4;
        extern "C" {
            fn mach_host_self() -> u32;
            fn host_page_size(host: u32, page_size: *mut u32) -> i32;
            fn host_statistics64(
                host: u32,
                flavor: i32,
                info: *mut vm_statistics64,
                count: *mut u32,
            ) -> i32;
        }

        unsafe {
            let host = mach_host_self();
            let mut page_size: u32 = 0;
            if host_page_size(host, &mut page_size) != 0 {
                return Err("host_page_size failed".into());
            }

            let mut stats = std::mem::zeroed::<vm_statistics64>();
            let mut count =
                (std::mem::size_of::<vm_statistics64>() / std::mem::size_of::<u32>()) as u32;

            if host_statistics64(host, HOST_VM_INFO64, &mut stats, &mut count) != 0 {
                return Err("host_statistics64 failed".into());
            }

            // "Available" ≈ free + inactive + purgeable (matches sysinfo behaviour).
            let pages = stats.free_count as u64
                + stats.inactive_count as u64
                + stats.purgeable_count as u64;
            Ok(pages * page_size as u64)
        }
    }

    let total = total_memory()?;
    let avail = available_memory()?;
    Ok((avail, total.saturating_sub(avail)))
}

// ── Linux ───────────────────────────────────────────────────────────
#[cfg(target_os = "linux")]
fn platform_memory() -> Result<(u64, u64), String> {
    let content = std::fs::read_to_string("/proc/meminfo")
        .map_err(|e| format!("Failed to read /proc/meminfo: {e}"))?;

    fn parse_kb(content: &str, key: &str) -> Option<u64> {
        content
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .map(|kb| kb * 1024) // convert KB → bytes
    }

    let total = parse_kb(&content, "MemTotal:").ok_or("MemTotal not found in /proc/meminfo")?;
    let available =
        parse_kb(&content, "MemAvailable:").ok_or("MemAvailable not found in /proc/meminfo")?;

    Ok((available, total.saturating_sub(available)))
}
