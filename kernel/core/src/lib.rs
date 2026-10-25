use gennie_config::Config;
pub use gennie_config::{Error, Result};
pub use gennie_db::{Database, Migration};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub shutdown: Shutdown,
}
#[derive(Clone, Default)]
pub struct Shutdown(Arc<AtomicBool>);
impl Shutdown {
    pub fn request(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn requested(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
pub trait AppModule: Send + Sync {
    fn name(&self) -> &'static str;
    fn active_by_default(&self) -> bool {
        true
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &[]
    }
    fn optional_dependencies(&self) -> &'static [&'static str] {
        &[]
    }
    fn migrations(&self) -> &'static [Migration] {
        &[]
    }
    fn start(&self, state: &AppState) -> Result<()>;
    fn stop(&self, state: &AppState) -> Result<()>;
}

/// Memoria residente del proceso indicado, consultada bajo demanda. None si no se puede medir.
pub fn process_resident_bytes(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        // ABI de proc_taskinfo en el SDK de macOS: seis u64 seguidos de doce i32.
        #[repr(C)]
        #[derive(Default)]
        struct TaskInfo {
            virtual_size: u64,
            resident_size: u64,
            times: [u64; 4],
            counters: [i32; 12],
        }
        #[link(name = "proc")]
        unsafe extern "C" {
            fn proc_pidinfo(
                pid: i32,
                flavor: i32,
                arg: u64,
                buffer: *mut std::ffi::c_void,
                size: i32,
            ) -> i32;
        }
        let pid = i32::try_from(pid).ok().filter(|pid| *pid > 0)?;
        let mut info = TaskInfo::default();
        let size = std::mem::size_of::<TaskInfo>() as i32;
        // El buffer tiene el tamaño y la alineación exigidos por PROC_PIDTASKINFO (4).
        let read = unsafe { proc_pidinfo(pid, 4, 0, (&mut info as *mut TaskInfo).cast(), size) };
        (read == size).then_some(info.resident_size)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = pid;
        None
    }
}

#[cfg(all(test, target_os = "macos"))]
mod memory_tests {
    #[test]
    fn resident_memory_reads_current_process_and_rejects_invalid_pid() {
        assert!(super::process_resident_bytes(std::process::id()).is_some_and(|n| n > 0));
        assert_eq!(super::process_resident_bytes(0), None);
        assert_eq!(super::process_resident_bytes(u32::MAX), None);
    }
}
