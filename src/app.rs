use crate::process::ProcessInfo;
use ratatui::widgets::TableState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Cpu,
    Memory,
    Name,
    Pid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Inactive,
    Editing,
    Applied,
}

pub struct AppState {
    pub table: TableState,
    pub sort_key: SortKey,
    pub sort_dir: SortDir,
    pub filter_mode: FilterMode,
    pub filter: String,
}

impl AppState {
    pub fn new() -> Self {
        let mut table = TableState::default();
        table.select(Some(0));
        Self {
            table,
            sort_key: SortKey::Pid,
            sort_dir: SortDir::Asc,
            filter_mode: FilterMode::Inactive,
            filter: String::new(),
        }
    }

    pub fn start_filter_edit(&mut self) {
        self.filter_mode = FilterMode::Editing;
    }

    pub fn confirm_filter(&mut self) {
        self.filter_mode = if self.filter.is_empty() {
            FilterMode::Inactive
        } else {
            FilterMode::Applied
        };
    }

    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filter_mode = FilterMode::Inactive;
    }

    pub fn toggle_sort(&mut self, key: SortKey) {
        if self.sort_key == key {
            self.sort_dir = match self.sort_dir {
                SortDir::Asc => SortDir::Desc,
                SortDir::Desc => SortDir::Asc,
            };
        } else {
            self.sort_key = key;
            self.sort_dir = match key {
                SortKey::Cpu | SortKey::Memory => SortDir::Desc,
                SortKey::Name | SortKey::Pid => SortDir::Asc,
            };
        }
    }

    pub fn select_next(&mut self, total: usize) {
        if total == 0 {
            self.table.select(None);
            return;
        }
        let next = match self.table.selected() {
            Some(i) if i + 1 < total => i + 1,
            Some(_) => total - 1,
            None => 0,
        };
        self.table.select(Some(next));
    }

    pub fn select_prev(&mut self, total: usize) {
        if total == 0 {
            self.table.select(None);
            return;
        }
        let prev = self.table.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
        self.table.select(Some(prev));
    }

    pub fn select_first(&mut self, total: usize) {
        if total == 0 {
            self.table.select(None);
        } else {
            self.table.select(Some(0));
        }
    }

    pub fn select_last(&mut self, total: usize) {
        if total == 0 {
            self.table.select(None);
        } else {
            self.table.select(Some(total - 1));
        }
    }

    pub fn page_down(&mut self, total: usize, page: usize) {
        if total == 0 {
            self.table.select(None);
            return;
        }
        let cur = self.table.selected().unwrap_or(0);
        let next = (cur + page).min(total - 1);
        self.table.select(Some(next));
    }

    pub fn page_up(&mut self, total: usize, page: usize) {
        if total == 0 {
            self.table.select(None);
            return;
        }
        let cur = self.table.selected().unwrap_or(0);
        self.table.select(Some(cur.saturating_sub(page)));
    }

    pub fn clamp_to(&mut self, total: usize) {
        if total == 0 {
            self.table.select(None);
        } else if let Some(i) = self.table.selected() {
            if i >= total {
                self.table.select(Some(total - 1));
            }
        } else {
            self.table.select(Some(0));
        }
    }
}

pub fn filter_indices(processes: &[ProcessInfo], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..processes.len()).collect();
    }
    let needle = filter.to_lowercase();
    processes
        .iter()
        .enumerate()
        .filter(|(_, p)| p.name.to_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect()
}

pub fn sort_processes(processes: &mut [ProcessInfo], key: SortKey, dir: SortDir) {
    use std::cmp::Ordering;
    processes.sort_by(|a, b| {
        let ord = match key {
            SortKey::Cpu => a.cpu.partial_cmp(&b.cpu).unwrap_or(Ordering::Equal),
            SortKey::Memory => a
                .memory_mb
                .partial_cmp(&b.memory_mb)
                .unwrap_or(Ordering::Equal),
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Pid => a.pid.cmp(&b.pid),
        };
        match dir {
            SortDir::Asc => ord,
            SortDir::Desc => ord.reverse(),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, name: &str, cpu: f32, mem: f64) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cpu,
            memory_mb: mem,
        }
    }

    #[test]
    fn default_sort_is_pid_asc_for_stable_rows() {
        let s = AppState::new();
        assert_eq!(s.sort_key, SortKey::Pid);
        assert_eq!(s.sort_dir, SortDir::Asc);
    }

    #[test]
    fn toggle_sort_flips_dir_on_same_key() {
        let mut s = AppState::new();
        s.toggle_sort(SortKey::Cpu);
        assert_eq!(s.sort_dir, SortDir::Desc);
        s.toggle_sort(SortKey::Cpu);
        assert_eq!(s.sort_dir, SortDir::Asc);
        s.toggle_sort(SortKey::Cpu);
        assert_eq!(s.sort_dir, SortDir::Desc);
    }

    #[test]
    fn toggle_sort_picks_sensible_default_dir_on_new_key() {
        let mut s = AppState::new();
        s.toggle_sort(SortKey::Name);
        assert_eq!(s.sort_key, SortKey::Name);
        assert_eq!(s.sort_dir, SortDir::Asc);
        s.toggle_sort(SortKey::Memory);
        assert_eq!(s.sort_key, SortKey::Memory);
        assert_eq!(s.sort_dir, SortDir::Desc);
        s.toggle_sort(SortKey::Pid);
        assert_eq!(s.sort_dir, SortDir::Asc);
    }

    #[test]
    fn sort_processes_by_cpu_desc() {
        let mut v = vec![
            proc(1, "a", 5.0, 100.0),
            proc(2, "b", 80.0, 50.0),
            proc(3, "c", 20.0, 200.0),
        ];
        sort_processes(&mut v, SortKey::Cpu, SortDir::Desc);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn sort_processes_by_name_asc_case_insensitive() {
        let mut v = vec![
            proc(1, "zebra", 0.0, 0.0),
            proc(2, "Alpha", 0.0, 0.0),
            proc(3, "beta", 0.0, 0.0),
        ];
        sort_processes(&mut v, SortKey::Name, SortDir::Asc);
        assert_eq!(
            v.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
            vec!["Alpha", "beta", "zebra"]
        );
    }

    #[test]
    fn sort_processes_by_pid_asc() {
        let mut v = vec![proc(30, "a", 0.0, 0.0), proc(2, "b", 0.0, 0.0), proc(10, "c", 0.0, 0.0)];
        sort_processes(&mut v, SortKey::Pid, SortDir::Asc);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![2, 10, 30]);
    }

    #[test]
    fn default_filter_is_inactive_and_empty() {
        let s = AppState::new();
        assert_eq!(s.filter_mode, FilterMode::Inactive);
        assert!(s.filter.is_empty());
    }

    #[test]
    fn confirm_filter_empty_goes_inactive() {
        let mut s = AppState::new();
        s.start_filter_edit();
        assert_eq!(s.filter_mode, FilterMode::Editing);
        s.confirm_filter();
        assert_eq!(s.filter_mode, FilterMode::Inactive);
    }

    #[test]
    fn confirm_filter_with_text_goes_applied() {
        let mut s = AppState::new();
        s.start_filter_edit();
        s.filter.push_str("chr");
        s.confirm_filter();
        assert_eq!(s.filter_mode, FilterMode::Applied);
        assert_eq!(s.filter, "chr");
    }

    #[test]
    fn clear_filter_resets() {
        let mut s = AppState::new();
        s.filter.push_str("noisy");
        s.filter_mode = FilterMode::Applied;
        s.clear_filter();
        assert!(s.filter.is_empty());
        assert_eq!(s.filter_mode, FilterMode::Inactive);
    }

    #[test]
    fn filter_indices_empty_returns_all() {
        let v = vec![
            proc(1, "a", 0.0, 0.0),
            proc(2, "b", 0.0, 0.0),
            proc(3, "c", 0.0, 0.0),
        ];
        assert_eq!(filter_indices(&v, ""), vec![0, 1, 2]);
    }

    #[test]
    fn filter_indices_case_insensitive_substring() {
        let v = vec![
            proc(1, "Chrome.exe", 0.0, 0.0),
            proc(2, "firefox", 0.0, 0.0),
            proc(3, "MyChrome-helper", 0.0, 0.0),
            proc(4, "notepad", 0.0, 0.0),
        ];
        assert_eq!(filter_indices(&v, "chrome"), vec![0, 2]);
        assert_eq!(filter_indices(&v, "FOX"), vec![1]);
    }

    #[test]
    fn filter_indices_no_match_returns_empty() {
        let v = vec![proc(1, "alpha", 0.0, 0.0), proc(2, "beta", 0.0, 0.0)];
        assert!(filter_indices(&v, "zzz").is_empty());
    }

    #[test]
    fn sort_processes_by_memory_desc() {
        let mut v = vec![
            proc(1, "a", 0.0, 100.0),
            proc(2, "b", 0.0, 500.0),
            proc(3, "c", 0.0, 250.0),
        ];
        sort_processes(&mut v, SortKey::Memory, SortDir::Desc);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![2, 3, 1]);
    }

    #[test]
    fn select_next_advances_and_clamps_at_end() {
        let mut s = AppState::new();
        s.select_next(3);
        assert_eq!(s.table.selected(), Some(1));
        s.select_next(3);
        assert_eq!(s.table.selected(), Some(2));
        s.select_next(3);
        assert_eq!(s.table.selected(), Some(2)); // no wrap
    }

    #[test]
    fn select_prev_stops_at_zero() {
        let mut s = AppState::new();
        s.select_next(5);
        s.select_next(5);
        assert_eq!(s.table.selected(), Some(2));
        s.select_prev(5);
        s.select_prev(5);
        s.select_prev(5);
        assert_eq!(s.table.selected(), Some(0));
    }

    #[test]
    fn page_down_and_up_jumps() {
        let mut s = AppState::new();
        s.page_down(100, 10);
        assert_eq!(s.table.selected(), Some(10));
        s.page_down(100, 10);
        assert_eq!(s.table.selected(), Some(20));
        s.page_up(100, 15);
        assert_eq!(s.table.selected(), Some(5));
    }

    #[test]
    fn first_last_handle_empty() {
        let mut s = AppState::new();
        s.select_last(0);
        assert_eq!(s.table.selected(), None);
        s.select_first(0);
        assert_eq!(s.table.selected(), None);
        s.select_last(10);
        assert_eq!(s.table.selected(), Some(9));
        s.select_first(10);
        assert_eq!(s.table.selected(), Some(0));
    }

    #[test]
    fn clamp_to_shrinks_selection_when_list_shrinks() {
        let mut s = AppState::new();
        s.select_last(50);
        assert_eq!(s.table.selected(), Some(49));
        s.clamp_to(10);
        assert_eq!(s.table.selected(), Some(9));
        s.clamp_to(0);
        assert_eq!(s.table.selected(), None);
    }
}
