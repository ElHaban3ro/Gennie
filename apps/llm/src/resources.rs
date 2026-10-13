// Lecturas locales acotadas para preguntas de recursos; no consultan procesos ajenos.
#[cfg(target_os = "macos")]
mod macos {
    use std::{process::Command, thread, time::Duration};

    unsafe extern "C" {
        fn mach_host_self() -> u32;
        static mach_task_self_: u32;
        fn host_processor_info(
            host: u32,
            flavor: i32,
            count: *mut u32,
            info: *mut *mut i32,
            size: *mut u32,
        ) -> i32;
        fn vm_deallocate(task: u32, address: usize, size: usize) -> i32;
    }

    fn cpu_ticks() -> Option<(u64, u64)> {
        let mut count = 0_u32;
        let mut info = std::ptr::null_mut();
        let mut size = 0_u32;
        // PROCESSOR_CPU_LOAD_INFO = 2; cada CPU entrega user, system, idle y nice.
        let status =
            unsafe { host_processor_info(mach_host_self(), 2, &mut count, &mut info, &mut size) };
        if status != 0 || info.is_null() || size < count.saturating_mul(4) {
            return None;
        }
        let values = unsafe { std::slice::from_raw_parts(info, size as usize) };
        let mut busy = 0_u64;
        let mut total = 0_u64;
        for ticks in values.chunks_exact(4).take(count as usize) {
            let user = ticks[0] as u32 as u64;
            let system = ticks[1] as u32 as u64;
            let idle = ticks[2] as u32 as u64;
            let nice = ticks[3] as u32 as u64;
            busy += user + system + nice;
            total += user + system + idle + nice;
        }
        unsafe {
            vm_deallocate(
                mach_task_self_,
                info as usize,
                size as usize * std::mem::size_of::<i32>(),
            )
        };
        Some((busy, total))
    }

    pub(super) fn snapshot() -> Option<String> {
        let first = cpu_ticks()?;
        thread::sleep(Duration::from_millis(250));
        let second = cpu_ticks()?;
        let busy = second.0.checked_sub(first.0)?;
        let total = second.1.checked_sub(first.1)?;
        if total == 0 {
            return None;
        }
        let mut details = format!(
            "CPU total ocupada durante 250 ms: {:.1}%.",
            100.0 * busy as f64 / total as f64
        );
        if let Ok(output) = Command::new("/usr/bin/memory_pressure").arg("-Q").output()
            && output.status.success()
        {
            let report = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = report
                .lines()
                .find(|line| line.starts_with("The system has "))
            {
                details.push(' ');
                details.push_str(line.trim());
            }
            if let Some(line) = report
                .lines()
                .find(|line| line.starts_with("System-wide memory free percentage:"))
            {
                details.push(' ');
                details.push_str(line.trim());
                details.push_str(
                    " (indicador de memory_pressure, no porcentaje de RAM libre física).",
                );
            }
        }
        Some(details)
    }
}

pub(crate) fn request_mentions_resources(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "cpu",
        "memoria",
        "ram",
        "memory",
        "recursos del sistema",
        "system resources",
    ]
    .iter()
    .any(|term| text.contains(term))
}

pub(crate) fn snapshot() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        macos::snapshot()
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_resource_question() {
        assert!(request_mentions_resources(
            "¿Cuánto CPU y memoria usa mi Mac ahora?"
        ));
        assert!(request_mentions_resources(
            "Consulta los recursos del sistema"
        ));
        assert!(!request_mentions_resources("Abre Música"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn samples_current_system_without_top() {
        let value = snapshot().expect("la lectura Mach de CPU debe estar disponible");
        assert!(value.contains("CPU total ocupada"));
        assert!(value.contains("System-wide memory free percentage:"));
    }
}
