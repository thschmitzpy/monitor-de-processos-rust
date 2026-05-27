use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System,
};

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
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let cpu_usage = self.sys.global_cpu_usage();

        let total_memory_gb = self.sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_memory_gb = self.sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;

        let processes: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().to_string(),
                cpu: p.cpu_usage(),
                memory_mb: p.memory() as f64 / 1024.0 / 1024.0,
            })
            .collect();

        SystemSnapshot {
            processes,
            cpu_usage,
            used_memory_gb,
            total_memory_gb,
        }
    }
}
