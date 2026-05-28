use crate::process::ProcessInfo;
use crossterm::event::KeyCode;
use ratatui::widgets::TableState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    KillConfirmed { pid: u32, name: String },
}

pub const REFRESH_STEPS_MS: &[u64] = &[250, 500, 1000, 2000, 5000];
pub const DEFAULT_REFRESH_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortKey {
    Cpu,
    Memory,
    Name,
    #[default]
    Pid,
    Io,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Inactive,
    Editing,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillPrompt {
    pub pid: u32,
    pub name: String,
}

pub struct AppState {
    pub table: TableState,
    pub sort_key: SortKey,
    pub sort_dir: SortDir,
    pub filter_mode: FilterMode,
    pub filter: String,
    pub show_details: bool,
    pub kill_prompt: Option<KillPrompt>,
    pub status_msg: Option<String>,
    pub paused: bool,
    pub refresh_ms: u64,
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
            show_details: false,
            kill_prompt: None,
            status_msg: None,
            paused: false,
            refresh_ms: DEFAULT_REFRESH_MS,
        }
    }

    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
    }

    pub fn request_kill(&mut self, pid: u32, name: String) {
        self.kill_prompt = Some(KillPrompt { pid, name });
    }

    pub fn cancel_kill(&mut self) {
        self.kill_prompt = None;
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn faster_refresh(&mut self) {
        let cur = self.refresh_ms;
        self.refresh_ms = REFRESH_STEPS_MS
            .iter()
            .rev()
            .find(|&&ms| ms < cur)
            .copied()
            .unwrap_or(REFRESH_STEPS_MS[0]);
    }

    pub fn slower_refresh(&mut self) {
        let cur = self.refresh_ms;
        let last = *REFRESH_STEPS_MS.last().expect("non-empty");
        self.refresh_ms = REFRESH_STEPS_MS
            .iter()
            .find(|&&ms| ms > cur)
            .copied()
            .unwrap_or(last);
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
                SortKey::Cpu | SortKey::Memory | SortKey::Io => SortDir::Desc,
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

pub fn handle_key(
    state: &mut AppState,
    code: KeyCode,
    total: usize,
    page_size: usize,
    selected: Option<&(u32, String)>,
) -> Action {
    if state.kill_prompt.is_some() {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let prompt = state.kill_prompt.take().expect("checked above");
                return Action::KillConfirmed {
                    pid: prompt.pid,
                    name: prompt.name,
                };
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => state.cancel_kill(),
            _ => {}
        }
        return Action::None;
    }

    if state.filter_mode == FilterMode::Editing {
        match code {
            KeyCode::Esc => state.clear_filter(),
            KeyCode::Enter => state.confirm_filter(),
            KeyCode::Backspace => {
                state.filter.pop();
            }
            KeyCode::Char(ch) => state.filter.push(ch),
            _ => {}
        }
        return Action::None;
    }

    match code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return Action::Quit,
        KeyCode::Char('/') => state.start_filter_edit(),
        KeyCode::Esc => {
            if state.status_msg.is_some() {
                state.status_msg = None;
            } else if state.show_details {
                state.show_details = false;
            } else {
                state.clear_filter();
            }
        }
        KeyCode::Char('c') | KeyCode::Char('C') => state.toggle_sort(SortKey::Cpu),
        KeyCode::Char('m') | KeyCode::Char('M') => state.toggle_sort(SortKey::Memory),
        KeyCode::Char('n') | KeyCode::Char('N') => state.toggle_sort(SortKey::Name),
        KeyCode::Char('p') | KeyCode::Char('P') => state.toggle_sort(SortKey::Pid),
        KeyCode::Char('i') | KeyCode::Char('I') => state.toggle_sort(SortKey::Io),
        KeyCode::Char('d') | KeyCode::Char('D') => state.toggle_details(),
        KeyCode::Char('k') | KeyCode::Char('K') | KeyCode::Delete => {
            if let Some((pid, name)) = selected {
                state.request_kill(*pid, name.clone());
            }
        }
        KeyCode::Char(' ') => state.toggle_pause(),
        KeyCode::Char('+') | KeyCode::Char('=') => state.faster_refresh(),
        KeyCode::Char('-') | KeyCode::Char('_') => state.slower_refresh(),
        KeyCode::Down => state.select_next(total),
        KeyCode::Up => state.select_prev(total),
        KeyCode::PageDown => state.page_down(total, page_size),
        KeyCode::PageUp => state.page_up(total, page_size),
        KeyCode::Home => state.select_first(total),
        KeyCode::End => state.select_last(total),
        _ => {}
    }
    Action::None
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
            SortKey::Io => {
                let a_io = a.disk_read_mb + a.disk_write_mb;
                let b_io = b.disk_read_mb + b.disk_write_mb;
                a_io.partial_cmp(&b_io).unwrap_or(Ordering::Equal)
            }
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
            disk_read_mb: 0.0,
            disk_write_mb: 0.0,
        }
    }

    fn proc_io(pid: u32, read: f64, write: f64) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: format!("p{pid}"),
            cpu: 0.0,
            memory_mb: 0.0,
            disk_read_mb: read,
            disk_write_mb: write,
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
    fn toggle_details_flips_bool() {
        let mut s = AppState::new();
        assert!(!s.show_details);
        s.toggle_details();
        assert!(s.show_details);
        s.toggle_details();
        assert!(!s.show_details);
    }

    #[test]
    fn request_kill_stores_pid_and_name() {
        let mut s = AppState::new();
        assert!(s.kill_prompt.is_none());
        s.request_kill(1234, "chrome.exe".to_string());
        let prompt = s.kill_prompt.as_ref().expect("prompt set");
        assert_eq!(prompt.pid, 1234);
        assert_eq!(prompt.name, "chrome.exe");
    }

    #[test]
    fn cancel_kill_clears_prompt() {
        let mut s = AppState::new();
        s.request_kill(1, "x".to_string());
        s.cancel_kill();
        assert!(s.kill_prompt.is_none());
    }

    // ---- handle_key tests ----

    #[test]
    fn handle_key_q_returns_quit() {
        let mut s = AppState::new();
        assert_eq!(
            handle_key(&mut s, KeyCode::Char('q'), 10, 10, None),
            Action::Quit
        );
        assert_eq!(
            handle_key(&mut s, KeyCode::Char('Q'), 10, 10, None),
            Action::Quit
        );
    }

    #[test]
    fn handle_key_kill_prompt_y_confirms_and_clears_prompt() {
        let mut s = AppState::new();
        s.request_kill(1234, "chrome".to_string());
        let act = handle_key(&mut s, KeyCode::Char('y'), 10, 10, None);
        assert_eq!(
            act,
            Action::KillConfirmed {
                pid: 1234,
                name: "chrome".to_string()
            }
        );
        assert!(s.kill_prompt.is_none());
    }

    #[test]
    fn handle_key_kill_prompt_n_cancels() {
        let mut s = AppState::new();
        s.request_kill(1, "x".to_string());
        let act = handle_key(&mut s, KeyCode::Char('n'), 10, 10, None);
        assert_eq!(act, Action::None);
        assert!(s.kill_prompt.is_none());
    }

    #[test]
    fn handle_key_kill_prompt_esc_cancels() {
        let mut s = AppState::new();
        s.request_kill(1, "x".to_string());
        let act = handle_key(&mut s, KeyCode::Esc, 10, 10, None);
        assert_eq!(act, Action::None);
        assert!(s.kill_prompt.is_none());
    }

    #[test]
    fn handle_key_kill_prompt_blocks_navigation_and_quit() {
        let mut s = AppState::new();
        s.select_first(10);
        s.request_kill(1, "x".to_string());

        let act = handle_key(&mut s, KeyCode::Down, 10, 10, None);
        assert_eq!(act, Action::None);
        assert_eq!(s.table.selected(), Some(0));

        let act = handle_key(&mut s, KeyCode::Char('q'), 10, 10, None);
        assert_eq!(act, Action::None);
        assert!(s.kill_prompt.is_some());
    }

    #[test]
    fn handle_key_filter_edit_appends_char_and_pops_backspace() {
        let mut s = AppState::new();
        s.start_filter_edit();
        handle_key(&mut s, KeyCode::Char('a'), 0, 10, None);
        handle_key(&mut s, KeyCode::Char('b'), 0, 10, None);
        assert_eq!(s.filter, "ab");
        handle_key(&mut s, KeyCode::Backspace, 0, 10, None);
        assert_eq!(s.filter, "a");
    }

    #[test]
    fn handle_key_filter_edit_q_is_typed_not_quit() {
        let mut s = AppState::new();
        s.start_filter_edit();
        let act = handle_key(&mut s, KeyCode::Char('q'), 0, 10, None);
        assert_eq!(act, Action::None);
        assert_eq!(s.filter, "q");
        assert_eq!(s.filter_mode, FilterMode::Editing);
    }

    #[test]
    fn handle_key_filter_edit_enter_confirms() {
        let mut s = AppState::new();
        s.start_filter_edit();
        s.filter.push_str("ch");
        handle_key(&mut s, KeyCode::Enter, 0, 10, None);
        assert_eq!(s.filter_mode, FilterMode::Applied);
        assert_eq!(s.filter, "ch");
    }

    #[test]
    fn handle_key_filter_edit_esc_clears() {
        let mut s = AppState::new();
        s.start_filter_edit();
        s.filter.push_str("x");
        handle_key(&mut s, KeyCode::Esc, 0, 10, None);
        assert_eq!(s.filter_mode, FilterMode::Inactive);
        assert!(s.filter.is_empty());
    }

    #[test]
    fn handle_key_slash_enters_filter_edit() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('/'), 0, 10, None);
        assert_eq!(s.filter_mode, FilterMode::Editing);
    }

    #[test]
    fn handle_key_d_toggles_details() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('d'), 0, 10, None);
        assert!(s.show_details);
        handle_key(&mut s, KeyCode::Char('d'), 0, 10, None);
        assert!(!s.show_details);
    }

    #[test]
    fn handle_key_k_opens_kill_prompt_for_selected() {
        let mut s = AppState::new();
        let sel = (5678u32, "firefox".to_string());
        handle_key(&mut s, KeyCode::Char('k'), 10, 10, Some(&sel));
        let p = s.kill_prompt.as_ref().expect("prompt set");
        assert_eq!(p.pid, 5678);
        assert_eq!(p.name, "firefox");
    }

    #[test]
    fn handle_key_delete_also_opens_kill_prompt() {
        let mut s = AppState::new();
        let sel = (1u32, "x".to_string());
        handle_key(&mut s, KeyCode::Delete, 10, 10, Some(&sel));
        assert!(s.kill_prompt.is_some());
    }

    #[test]
    fn handle_key_k_without_selection_is_noop() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('k'), 0, 10, None);
        assert!(s.kill_prompt.is_none());
    }

    #[test]
    fn handle_key_esc_priority_status_msg_first() {
        let mut s = AppState::new();
        s.status_msg = Some("hi".to_string());
        s.show_details = true;
        s.filter.push_str("x");
        s.filter_mode = FilterMode::Applied;
        handle_key(&mut s, KeyCode::Esc, 0, 10, None);
        assert!(s.status_msg.is_none());
        assert!(s.show_details, "details should still be open");
        assert_eq!(s.filter, "x", "filter should be untouched");
    }

    #[test]
    fn handle_key_esc_priority_details_second() {
        let mut s = AppState::new();
        s.show_details = true;
        s.filter.push_str("x");
        s.filter_mode = FilterMode::Applied;
        handle_key(&mut s, KeyCode::Esc, 0, 10, None);
        assert!(!s.show_details);
        assert_eq!(s.filter, "x", "filter should be untouched");
    }

    #[test]
    fn handle_key_esc_priority_filter_last() {
        let mut s = AppState::new();
        s.filter.push_str("x");
        s.filter_mode = FilterMode::Applied;
        handle_key(&mut s, KeyCode::Esc, 0, 10, None);
        assert!(s.filter.is_empty());
        assert_eq!(s.filter_mode, FilterMode::Inactive);
    }

    #[test]
    fn handle_key_arrows_navigate() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Down, 5, 10, None);
        assert_eq!(s.table.selected(), Some(1));
        handle_key(&mut s, KeyCode::Up, 5, 10, None);
        assert_eq!(s.table.selected(), Some(0));
    }

    #[test]
    fn handle_key_page_keys_jump() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::PageDown, 100, 10, None);
        assert_eq!(s.table.selected(), Some(10));
        handle_key(&mut s, KeyCode::End, 100, 10, None);
        assert_eq!(s.table.selected(), Some(99));
        handle_key(&mut s, KeyCode::Home, 100, 10, None);
        assert_eq!(s.table.selected(), Some(0));
    }

    #[test]
    fn default_state_is_running_at_default_interval() {
        let s = AppState::new();
        assert!(!s.paused);
        assert_eq!(s.refresh_ms, DEFAULT_REFRESH_MS);
    }

    #[test]
    fn toggle_pause_flips_bool() {
        let mut s = AppState::new();
        s.toggle_pause();
        assert!(s.paused);
        s.toggle_pause();
        assert!(!s.paused);
    }

    #[test]
    fn faster_refresh_steps_down_through_array() {
        let mut s = AppState::new();
        assert_eq!(s.refresh_ms, 1000);
        s.faster_refresh();
        assert_eq!(s.refresh_ms, 500);
        s.faster_refresh();
        assert_eq!(s.refresh_ms, 250);
    }

    #[test]
    fn faster_refresh_clamps_at_minimum() {
        let mut s = AppState::new();
        s.refresh_ms = REFRESH_STEPS_MS[0];
        s.faster_refresh();
        assert_eq!(s.refresh_ms, REFRESH_STEPS_MS[0]);
    }

    #[test]
    fn slower_refresh_steps_up_through_array() {
        let mut s = AppState::new();
        assert_eq!(s.refresh_ms, 1000);
        s.slower_refresh();
        assert_eq!(s.refresh_ms, 2000);
        s.slower_refresh();
        assert_eq!(s.refresh_ms, 5000);
    }

    #[test]
    fn slower_refresh_clamps_at_maximum() {
        let mut s = AppState::new();
        let max = *REFRESH_STEPS_MS.last().unwrap();
        s.refresh_ms = max;
        s.slower_refresh();
        assert_eq!(s.refresh_ms, max);
    }

    #[test]
    fn handle_key_space_toggles_pause() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char(' '), 0, 10, None);
        assert!(s.paused);
        handle_key(&mut s, KeyCode::Char(' '), 0, 10, None);
        assert!(!s.paused);
    }

    #[test]
    fn handle_key_plus_and_equals_make_faster() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('+'), 0, 10, None);
        assert_eq!(s.refresh_ms, 500);
        handle_key(&mut s, KeyCode::Char('='), 0, 10, None);
        assert_eq!(s.refresh_ms, 250);
    }

    #[test]
    fn handle_key_minus_makes_slower() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('-'), 0, 10, None);
        assert_eq!(s.refresh_ms, 2000);
    }

    #[test]
    fn handle_key_kill_prompt_blocks_pause_and_speed_keys() {
        let mut s = AppState::new();
        s.request_kill(1, "x".to_string());
        let pre_paused = s.paused;
        let pre_ms = s.refresh_ms;
        handle_key(&mut s, KeyCode::Char(' '), 0, 10, None);
        handle_key(&mut s, KeyCode::Char('+'), 0, 10, None);
        handle_key(&mut s, KeyCode::Char('-'), 0, 10, None);
        assert_eq!(s.paused, pre_paused);
        assert_eq!(s.refresh_ms, pre_ms);
        assert!(s.kill_prompt.is_some());
    }

    #[test]
    fn handle_key_filter_edit_space_is_typed_not_pause() {
        let mut s = AppState::new();
        s.start_filter_edit();
        handle_key(&mut s, KeyCode::Char(' '), 0, 10, None);
        assert!(!s.paused);
        assert_eq!(s.filter, " ");
    }

    #[test]
    fn handle_key_sort_keys_change_sort_column() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('c'), 0, 10, None);
        assert_eq!(s.sort_key, SortKey::Cpu);
        handle_key(&mut s, KeyCode::Char('m'), 0, 10, None);
        assert_eq!(s.sort_key, SortKey::Memory);
        handle_key(&mut s, KeyCode::Char('n'), 0, 10, None);
        assert_eq!(s.sort_key, SortKey::Name);
        handle_key(&mut s, KeyCode::Char('p'), 0, 10, None);
        assert_eq!(s.sort_key, SortKey::Pid);
        handle_key(&mut s, KeyCode::Char('i'), 0, 10, None);
        assert_eq!(s.sort_key, SortKey::Io);
    }

    #[test]
    fn sort_processes_by_io_desc_uses_read_plus_write() {
        let mut v = vec![
            proc_io(1, 10.0, 0.0),  // total 10
            proc_io(2, 0.0, 50.0),  // total 50
            proc_io(3, 30.0, 30.0), // total 60
        ];
        sort_processes(&mut v, SortKey::Io, SortDir::Desc);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn sort_processes_by_io_asc_uses_read_plus_write() {
        let mut v = vec![
            proc_io(1, 30.0, 30.0),
            proc_io(2, 0.0, 50.0),
            proc_io(3, 10.0, 0.0),
        ];
        sort_processes(&mut v, SortKey::Io, SortDir::Asc);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![3, 2, 1]);
    }

    #[test]
    fn toggle_sort_io_defaults_to_desc() {
        let mut s = AppState::new();
        s.toggle_sort(SortKey::Io);
        assert_eq!(s.sort_key, SortKey::Io);
        assert_eq!(s.sort_dir, SortDir::Desc);
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
