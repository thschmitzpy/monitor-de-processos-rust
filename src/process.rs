use std::collections::VecDeque;
use std::time::Instant;
use sysinfo::{
    CpuRefreshKind, MemoryRefreshKind, Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind,
    System,
};

pub const HISTORY_CAPACITY: usize = 120;

pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub cpu: f32,
    pub memory_mb: f64,
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
}

pub struct ProcessDetail {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub name: String,
    pub exe: Option<String>,
    pub cmd: String,
    pub cwd: Option<String>,
    pub status: String,
    pub run_time_secs: u64,
    pub cpu: f32,
    pub memory_mb: f64,
    pub virtual_memory_mb: f64,
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
}

pub struct SystemSnapshot {
    pub processes: Vec<ProcessInfo>,
    pub cpu_usage: f32,
    pub used_memory_gb: f64,
    pub total_memory_gb: f64,
    pub used_swap_gb: f64,
    pub total_swap_gb: f64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub host_name: Option<String>,
    pub cpu_brand: String,
    pub cpu_count: usize,
}

pub struct Collector {
    sys: System,
    info: SystemInfo,
    last_snapshot: Option<Instant>,
}

impl Collector {
    pub fn new() -> Self {
        let sys = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything())
                .with_processes(ProcessRefreshKind::everything()),
        );
        let cpu_brand = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        let info = SystemInfo {
            os_name: System::name(),
            os_version: System::os_version(),
            kernel_version: System::kernel_version(),
            host_name: System::host_name(),
            cpu_brand,
            cpu_count: sys.cpus().len(),
        };
        Self {
            sys,
            info,
            last_snapshot: None,
        }
    }

    pub fn info(&self) -> &SystemInfo {
        &self.info
    }

    pub fn snapshot(&mut self) -> SystemSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);

        let now = Instant::now();
        let elapsed_secs = self
            .last_snapshot
            .map(|t| now.duration_since(t).as_secs_f64())
            .filter(|s| *s > 0.001);
        self.last_snapshot = Some(now);

        let cpu_usage = self.sys.global_cpu_usage();

        let total_memory_gb = self.sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_memory_gb = self.sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let total_swap_gb = self.sys.total_swap() as f64 / 1024.0 / 1024.0 / 1024.0;
        let used_swap_gb = self.sys.used_swap() as f64 / 1024.0 / 1024.0 / 1024.0;
        let uptime_secs = System::uptime();

        let processes: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            .map(|p| {
                let du = p.disk_usage();
                let (read_rate, write_rate) = match elapsed_secs {
                    Some(secs) => (
                        (du.read_bytes as f64 / 1024.0 / 1024.0) / secs,
                        (du.written_bytes as f64 / 1024.0 / 1024.0) / secs,
                    ),
                    None => (0.0, 0.0),
                };
                ProcessInfo {
                    pid: p.pid().as_u32(),
                    ppid: p.parent().map(|pp| pp.as_u32()),
                    name: p.name().to_string_lossy().to_string(),
                    cpu: p.cpu_usage(),
                    memory_mb: p.memory() as f64 / 1024.0 / 1024.0,
                    disk_read_mb: read_rate,
                    disk_write_mb: write_rate,
                }
            })
            .collect();

        SystemSnapshot {
            processes,
            cpu_usage,
            used_memory_gb,
            total_memory_gb,
            used_swap_gb,
            total_swap_gb,
            uptime_secs,
        }
    }

    pub fn detail(&self, pid: u32) -> Option<ProcessDetail> {
        let p = self.sys.process(Pid::from(pid as usize))?;
        let cmd = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let disk = p.disk_usage();
        Some(ProcessDetail {
            pid,
            ppid: p.parent().map(|pp| pp.as_u32()),
            name: p.name().to_string_lossy().to_string(),
            exe: p.exe().map(|e| e.display().to_string()),
            cmd,
            cwd: p.cwd().map(|c| c.display().to_string()),
            status: format!("{}", p.status()),
            run_time_secs: p.run_time(),
            cpu: p.cpu_usage(),
            memory_mb: p.memory() as f64 / 1024.0 / 1024.0,
            virtual_memory_mb: p.virtual_memory() as f64 / 1024.0 / 1024.0,
            disk_read_mb: disk.total_read_bytes as f64 / 1024.0 / 1024.0,
            disk_write_mb: disk.total_written_bytes as f64 / 1024.0 / 1024.0,
        })
    }

    pub fn kill(&self, pid: u32) -> bool {
        self.sys
            .process(Pid::from(pid as usize))
            .map(|p| p.kill())
            .unwrap_or(false)
    }
}

pub struct History {
    cpu: VecDeque<u64>,
    ram_pct: VecDeque<u64>,
    capacity: usize,
}

impl History {
    pub fn new() -> Self {
        Self::with_capacity(HISTORY_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cpu: VecDeque::with_capacity(capacity),
            ram_pct: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, snapshot: &SystemSnapshot) {
        let cpu = snapshot.cpu_usage.clamp(0.0, 100.0).round() as u64;
        let ram = if snapshot.total_memory_gb > 0.0 {
            ((snapshot.used_memory_gb / snapshot.total_memory_gb) * 100.0)
                .clamp(0.0, 100.0)
                .round() as u64
        } else {
            0
        };
        push_bounded(&mut self.cpu, cpu, self.capacity);
        push_bounded(&mut self.ram_pct, ram, self.capacity);
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn cpu(&self) -> Vec<u64> {
        self.cpu.iter().copied().collect()
    }

    pub fn ram_pct(&self) -> Vec<u64> {
        self.ram_pct.iter().copied().collect()
    }
}

fn push_bounded(buf: &mut VecDeque<u64>, value: u64, capacity: usize) {
    if buf.len() == capacity {
        buf.pop_front();
    }
    buf.push_back(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(cpu: f32, used_gb: f64, total_gb: f64) -> SystemSnapshot {
        SystemSnapshot {
            processes: vec![],
            cpu_usage: cpu,
            used_memory_gb: used_gb,
            total_memory_gb: total_gb,
            used_swap_gb: 0.0,
            total_swap_gb: 0.0,
            uptime_secs: 0,
        }
    }

    #[test]
    fn history_starts_empty() {
        let h = History::new();
        assert!(h.cpu().is_empty());
        assert!(h.ram_pct().is_empty());
        assert_eq!(h.capacity(), HISTORY_CAPACITY);
    }

    #[test]
    fn history_push_appends_samples_and_computes_ram_pct() {
        let mut h = History::with_capacity(10);
        h.push(&snap(10.0, 4.0, 8.0));
        h.push(&snap(20.0, 6.0, 8.0));
        assert_eq!(h.cpu(), vec![10, 20]);
        assert_eq!(h.ram_pct(), vec![50, 75]);
    }

    #[test]
    fn history_evicts_oldest_when_at_capacity() {
        let mut h = History::with_capacity(3);
        for cpu in [1.0, 2.0, 3.0, 4.0, 5.0] {
            h.push(&snap(cpu, 0.0, 1.0));
        }
        assert_eq!(h.cpu(), vec![3, 4, 5]);
    }

    #[test]
    fn history_clamps_cpu_to_0_100_range() {
        let mut h = History::with_capacity(3);
        h.push(&snap(150.0, 0.0, 1.0));
        h.push(&snap(-5.0, 0.0, 1.0));
        h.push(&snap(42.6, 0.0, 1.0));
        assert_eq!(h.cpu(), vec![100, 0, 43]);
    }

    #[test]
    fn history_zero_total_memory_yields_zero_ram_pct() {
        let mut h = History::with_capacity(1);
        h.push(&snap(0.0, 4.0, 0.0));
        assert_eq!(h.ram_pct(), vec![0]);
    }
}
