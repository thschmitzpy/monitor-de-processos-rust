use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};

pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory_mb: f64,
}

pub struct SystemSnapshot {
    pub processes: Vec<ProcessInfo>,
    pub cpu_usage: f32,
    pub used_memory_gb: f64,
    pub total_memory_gb: f64,
}

pub struct Collector {
    sys: System,
}

impl Collector {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything()),
        );
        Self { sys }
    }

    pub fn snapshot(&mut self) -> SystemSnapshot {
        self.sys.refresh_all();

        let cpu_count = self.sys.cpus().len() as f32;
        let cpu_usage = self.sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / cpu_count;

        let total_memory_gb = self.sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_memory_gb = self.sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

        let mut processes: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu: p.cpu_usage() / cpu_count,
                memory_mb: p.memory() as f64 / 1024.0 / 1024.0,
            })
            .collect();

        processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));

        SystemSnapshot {
            processes,
            cpu_usage,
            used_memory_gb,
            total_memory_gb,
        }
    }
}
