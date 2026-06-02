use crate::process::ProcessInfo;
use crossterm::event::KeyCode;
use ratatui::widgets::TableState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Flat,
    Tree,
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
    pub sort_frozen: bool,
    pub frozen_order: Vec<u32>,
    pub frozen_tree_order: HashMap<Option<u32>, Vec<u32>>,
    pub view_mode: ViewMode,
    pub collapsed: HashSet<u32>,
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
            sort_frozen: false,
            frozen_order: Vec::new(),
            frozen_tree_order: HashMap::new(),
            view_mode: ViewMode::Flat,
            collapsed: HashSet::new(),
        }
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Flat => ViewMode::Tree,
            ViewMode::Tree => ViewMode::Flat,
        };
        self.collapsed.clear();
        self.frozen_order.clear();
        self.frozen_tree_order.clear();
    }

    pub fn toggle_collapsed(&mut self, pid: u32) {
        if !self.collapsed.insert(pid) {
            self.collapsed.remove(&pid);
        }
    }

    pub fn toggle_freeze(&mut self) {
        self.sort_frozen = !self.sort_frozen;
        if !self.sort_frozen {
            self.frozen_order.clear();
            self.frozen_tree_order.clear();
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
        self.frozen_order.clear();
        self.frozen_tree_order.clear();
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
        KeyCode::Char('f') | KeyCode::Char('F') => state.toggle_freeze(),
        KeyCode::Char('t') | KeyCode::Char('T') => state.toggle_view_mode(),
        KeyCode::Enter => {
            if let (ViewMode::Tree, Some((pid, _))) = (state.view_mode, selected) {
                state.toggle_collapsed(*pid);
            }
        }
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

pub fn stable_reorder(processes: &mut Vec<ProcessInfo>, order: &mut Vec<u32>) {
    use std::collections::HashMap;
    let mut by_pid: HashMap<u32, ProcessInfo> = processes.drain(..).map(|p| (p.pid, p)).collect();
    let mut new_order: Vec<u32> = Vec::with_capacity(by_pid.len());
    let mut new_list: Vec<ProcessInfo> = Vec::with_capacity(by_pid.len());

    for pid in order.iter() {
        if let Some(p) = by_pid.remove(pid) {
            new_order.push(*pid);
            new_list.push(p);
        }
    }

    let mut leftovers: Vec<ProcessInfo> = by_pid.into_values().collect();
    leftovers.sort_by_key(|p| p.pid);
    for p in leftovers {
        new_order.push(p.pid);
        new_list.push(p);
    }

    *order = new_order;
    *processes = new_list;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchKind {
    Pipe,
    Space,
    Tee,
    Corner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub idx: usize,
    pub depth: usize,
    pub branches: Vec<BranchKind>,
    pub has_children: bool,
    pub is_collapsed: bool,
    pub collapsed_count: usize,
}

pub fn build_flat_view(visible: &[usize]) -> Vec<TreeRow> {
    visible
        .iter()
        .map(|&i| TreeRow {
            idx: i,
            depth: 0,
            branches: Vec::new(),
            has_children: false,
            is_collapsed: false,
            collapsed_count: 0,
        })
        .collect()
}

pub fn build_tree_view(
    processes: &[ProcessInfo],
    sort_key: SortKey,
    sort_dir: SortDir,
    collapsed: &HashSet<u32>,
    filter: &str,
    frozen: Option<&HashMap<Option<u32>, Vec<u32>>>,
) -> Vec<TreeRow> {
    let n = processes.len();
    if n == 0 {
        return Vec::new();
    }

    let pid_to_idx: HashMap<u32, usize> = processes
        .iter()
        .enumerate()
        .map(|(i, p)| (p.pid, i))
        .collect();

    let mut parent_of: Vec<Option<usize>> = vec![None; n];
    for (idx, p) in processes.iter().enumerate() {
        if let Some(ppid) = p.ppid
            && let Some(&pidx) = pid_to_idx.get(&ppid)
            && pidx != idx
        {
            parent_of[idx] = Some(pidx);
        }
    }

    // Quebra ciclos (PID recycle, dados inconsistentes): qualquer nó cuja
    // cadeia de ancestrais voltar a ele mesmo vira raiz.
    let mut settled = vec![false; n];
    for start in 0..n {
        if settled[start] {
            continue;
        }
        let mut path: Vec<usize> = Vec::new();
        let mut path_set: HashSet<usize> = HashSet::new();
        let mut cur = start;
        loop {
            if settled[cur] {
                break;
            }
            if !path_set.insert(cur) {
                parent_of[cur] = None;
                break;
            }
            path.push(cur);
            match parent_of[cur] {
                Some(p) => cur = p,
                None => break,
            }
        }
        for node in path {
            settled[node] = true;
        }
    }

    let mut children_of: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    for (idx, parent) in parent_of.iter().enumerate() {
        match parent {
            Some(pidx) => children_of.entry(*pidx).or_default().push(idx),
            None => roots.push(idx),
        }
    }

    for (parent_idx, kids) in children_of.iter_mut() {
        let parent_pid = Some(processes[*parent_idx].pid);
        let frozen_kids = frozen.and_then(|f| f.get(&parent_pid));
        order_siblings(kids, processes, sort_key, sort_dir, frozen_kids);
    }
    {
        let frozen_roots = frozen.and_then(|f| f.get(&None));
        order_siblings(&mut roots, processes, sort_key, sort_dir, frozen_roots);
    }

    let needle = filter.to_lowercase();
    let mut visible: Vec<bool> = vec![false; n];
    for &r in &roots {
        mark_visible(r, &needle, processes, &children_of, &mut visible);
    }

    let mut out: Vec<TreeRow> = Vec::new();
    let mut prefix: Vec<BranchKind> = Vec::new();
    let visible_roots: Vec<usize> = roots.iter().copied().filter(|&i| visible[i]).collect();
    let last_root = visible_roots.len().saturating_sub(1);
    for (i, &r) in visible_roots.iter().enumerate() {
        dfs_tree(
            r,
            i == last_root,
            true,
            &mut prefix,
            processes,
            &children_of,
            &visible,
            collapsed,
            &mut out,
        );
    }
    out
}

fn order_siblings(
    idxs: &mut Vec<usize>,
    processes: &[ProcessInfo],
    key: SortKey,
    dir: SortDir,
    frozen: Option<&Vec<u32>>,
) {
    let Some(frozen) = frozen else {
        sort_sibling_indices(idxs, processes, key, dir);
        return;
    };
    let mut by_pid: HashMap<u32, usize> =
        idxs.iter().map(|&i| (processes[i].pid, i)).collect();
    let mut kept: Vec<usize> = Vec::with_capacity(idxs.len());
    for pid in frozen {
        if let Some(i) = by_pid.remove(pid) {
            kept.push(i);
        }
    }
    let mut leftovers: Vec<usize> = by_pid.into_values().collect();
    sort_sibling_indices(&mut leftovers, processes, key, dir);
    kept.extend(leftovers);
    *idxs = kept;
}

pub fn capture_tree_order(
    rows: &[TreeRow],
    processes: &[ProcessInfo],
) -> HashMap<Option<u32>, Vec<u32>> {
    let mut result: HashMap<Option<u32>, Vec<u32>> = HashMap::new();
    let mut parent_stack: Vec<u32> = Vec::new();
    for row in rows {
        parent_stack.truncate(row.depth);
        let pid = processes[row.idx].pid;
        let parent_key = if row.depth == 0 {
            None
        } else {
            parent_stack.last().copied()
        };
        result.entry(parent_key).or_default().push(pid);
        parent_stack.push(pid);
    }
    result
}

fn sort_sibling_indices(
    idxs: &mut [usize],
    processes: &[ProcessInfo],
    key: SortKey,
    dir: SortDir,
) {
    use std::cmp::Ordering;
    idxs.sort_by(|&a, &b| {
        let pa = &processes[a];
        let pb = &processes[b];
        let ord = match key {
            SortKey::Cpu => pa.cpu.partial_cmp(&pb.cpu).unwrap_or(Ordering::Equal),
            SortKey::Memory => pa.memory_mb.partial_cmp(&pb.memory_mb).unwrap_or(Ordering::Equal),
            SortKey::Name => pa.name.to_lowercase().cmp(&pb.name.to_lowercase()),
            SortKey::Pid => pa.pid.cmp(&pb.pid),
            SortKey::Io => {
                let a_io = pa.disk_read_mb + pa.disk_write_mb;
                let b_io = pb.disk_read_mb + pb.disk_write_mb;
                a_io.partial_cmp(&b_io).unwrap_or(Ordering::Equal)
            }
        };
        match dir {
            SortDir::Asc => ord,
            SortDir::Desc => ord.reverse(),
        }
    });
}

fn mark_visible(
    idx: usize,
    needle: &str,
    processes: &[ProcessInfo],
    children_of: &HashMap<usize, Vec<usize>>,
    visible: &mut [bool],
) -> bool {
    let self_match = needle.is_empty() || processes[idx].name.to_lowercase().contains(needle);
    let mut any_child = false;
    if let Some(kids) = children_of.get(&idx) {
        for &c in kids {
            if mark_visible(c, needle, processes, children_of, visible) {
                any_child = true;
            }
        }
    }
    let v = self_match || any_child;
    visible[idx] = v;
    v
}

fn count_visible_descendants(
    idx: usize,
    children_of: &HashMap<usize, Vec<usize>>,
    visible: &[bool],
) -> usize {
    let Some(kids) = children_of.get(&idx) else {
        return 0;
    };
    let mut total = 0;
    for &c in kids {
        if !visible[c] {
            continue;
        }
        total += 1;
        total += count_visible_descendants(c, children_of, visible);
    }
    total
}

#[allow(clippy::too_many_arguments)]
fn dfs_tree(
    idx: usize,
    is_last: bool,
    is_root: bool,
    prefix: &mut Vec<BranchKind>,
    processes: &[ProcessInfo],
    children_of: &HashMap<usize, Vec<usize>>,
    visible: &[bool],
    collapsed: &HashSet<u32>,
    out: &mut Vec<TreeRow>,
) {
    let pid = processes[idx].pid;
    let visible_kids: Vec<usize> = children_of
        .get(&idx)
        .map(|kids| kids.iter().copied().filter(|&c| visible[c]).collect())
        .unwrap_or_default();
    let has_children = !visible_kids.is_empty();
    let is_collapsed = has_children && collapsed.contains(&pid);
    let collapsed_count = if is_collapsed {
        count_visible_descendants(idx, children_of, visible)
    } else {
        0
    };

    let mut branches = prefix.clone();
    if !is_root {
        branches.push(if is_last { BranchKind::Corner } else { BranchKind::Tee });
    }
    let depth = branches.len();

    out.push(TreeRow {
        idx,
        depth,
        branches,
        has_children,
        is_collapsed,
        collapsed_count,
    });

    if is_collapsed {
        return;
    }

    let pushed = if !is_root {
        prefix.push(if is_last {
            BranchKind::Space
        } else {
            BranchKind::Pipe
        });
        true
    } else {
        false
    };

    let last_child = visible_kids.len().saturating_sub(1);
    for (i, &c) in visible_kids.iter().enumerate() {
        dfs_tree(
            c,
            i == last_child,
            false,
            prefix,
            processes,
            children_of,
            visible,
            collapsed,
            out,
        );
    }

    if pushed {
        prefix.pop();
    }
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
            ppid: None,
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
            ppid: None,
            name: format!("p{pid}"),
            cpu: 0.0,
            memory_mb: 0.0,
            disk_read_mb: read,
            disk_write_mb: write,
        }
    }

    fn proc_with_parent(pid: u32, ppid: Option<u32>, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            name: name.to_string(),
            cpu: 0.0,
            memory_mb: 0.0,
            disk_read_mb: 0.0,
            disk_write_mb: 0.0,
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

    // ---- sort freeze tests ----

    #[test]
    fn default_state_is_not_frozen() {
        let s = AppState::new();
        assert!(!s.sort_frozen);
        assert!(s.frozen_order.is_empty());
    }

    #[test]
    fn toggle_freeze_flips_bool_and_clears_order_on_off() {
        let mut s = AppState::new();
        s.toggle_freeze();
        assert!(s.sort_frozen);
        s.frozen_order = vec![1, 2, 3];
        s.toggle_freeze();
        assert!(!s.sort_frozen);
        assert!(s.frozen_order.is_empty());
    }

    #[test]
    fn handle_key_f_toggles_freeze() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('f'), 0, 10, None);
        assert!(s.sort_frozen);
        handle_key(&mut s, KeyCode::Char('F'), 0, 10, None);
        assert!(!s.sort_frozen);
    }

    #[test]
    fn handle_key_f_in_filter_edit_is_typed_not_toggle() {
        let mut s = AppState::new();
        s.start_filter_edit();
        handle_key(&mut s, KeyCode::Char('f'), 0, 10, None);
        assert!(!s.sort_frozen);
        assert_eq!(s.filter, "f");
    }

    #[test]
    fn toggle_sort_invalidates_frozen_order() {
        let mut s = AppState::new();
        s.toggle_freeze();
        s.frozen_order = vec![10, 20, 30];
        s.toggle_sort(SortKey::Cpu);
        assert!(s.sort_frozen, "ainda travado");
        assert!(s.frozen_order.is_empty(), "ordem foi invalidada");
    }

    #[test]
    fn stable_reorder_preserves_known_pid_order() {
        let mut v = vec![
            proc(3, "c", 0.0, 0.0),
            proc(1, "a", 0.0, 0.0),
            proc(2, "b", 0.0, 0.0),
        ];
        let mut order = vec![1u32, 2, 3];
        stable_reorder(&mut v, &mut order);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(order, vec![1, 2, 3]);
    }

    #[test]
    fn stable_reorder_appends_new_pids_sorted_at_end() {
        let mut v = vec![
            proc(7, "g", 0.0, 0.0), // novo
            proc(1, "a", 0.0, 0.0),
            proc(5, "e", 0.0, 0.0), // novo
            proc(2, "b", 0.0, 0.0),
        ];
        let mut order = vec![1u32, 2];
        stable_reorder(&mut v, &mut order);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![1, 2, 5, 7]);
        assert_eq!(order, vec![1, 2, 5, 7]);
    }

    #[test]
    fn stable_reorder_drops_dead_pids_from_order() {
        let mut v = vec![proc(1, "a", 0.0, 0.0), proc(3, "c", 0.0, 0.0)];
        let mut order = vec![1u32, 2, 3]; // pid 2 morreu
        stable_reorder(&mut v, &mut order);
        assert_eq!(v.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(order, vec![1, 3]);
    }

    #[test]
    fn stable_reorder_empty_processes_yields_empty_order() {
        let mut v: Vec<ProcessInfo> = vec![];
        let mut order = vec![1u32, 2, 3];
        stable_reorder(&mut v, &mut order);
        assert!(v.is_empty());
        assert!(order.is_empty());
    }

    #[test]
    fn handle_key_kill_prompt_blocks_freeze_toggle() {
        let mut s = AppState::new();
        s.request_kill(1, "x".to_string());
        handle_key(&mut s, KeyCode::Char('f'), 0, 10, None);
        assert!(!s.sort_frozen);
        assert!(s.kill_prompt.is_some());
    }


    fn pids(rows: &[TreeRow], processes: &[ProcessInfo]) -> Vec<u32> {
        rows.iter().map(|r| processes[r.idx].pid).collect()
    }

    #[test]
    fn build_tree_view_empty_returns_empty() {
        let v: Vec<ProcessInfo> = vec![];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert!(rows.is_empty());
    }

    #[test]
    fn build_tree_view_flat_when_no_relations() {
        let v = vec![
            proc_with_parent(3, None, "c"),
            proc_with_parent(1, None, "a"),
            proc_with_parent(2, None, "b"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows, &v), vec![1, 2, 3]);
        for row in &rows {
            assert_eq!(row.depth, 0);
            assert!(row.branches.is_empty());
            assert!(!row.has_children);
        }
    }

    #[test]
    fn build_tree_view_nests_children_under_parent() {
        let v = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "child_a"),
            proc_with_parent(3, Some(1), "child_b"),
            proc_with_parent(4, Some(2), "grand"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows, &v), vec![1, 2, 4, 3]);

        let by_pid: HashMap<u32, &TreeRow> = rows
            .iter()
            .map(|r| (v[r.idx].pid, r))
            .collect();

        assert_eq!(by_pid[&1].depth, 0);
        assert_eq!(by_pid[&1].branches, vec![]);
        assert!(by_pid[&1].has_children);

        assert_eq!(by_pid[&2].depth, 1);
        assert_eq!(by_pid[&2].branches, vec![BranchKind::Tee]);
        assert!(by_pid[&2].has_children);

        assert_eq!(by_pid[&3].depth, 1);
        assert_eq!(by_pid[&3].branches, vec![BranchKind::Corner]);

        assert_eq!(by_pid[&4].depth, 2);
        assert_eq!(by_pid[&4].branches, vec![BranchKind::Pipe, BranchKind::Corner]);
    }

    #[test]
    fn build_tree_view_orphan_parent_becomes_root() {
        let v = vec![
            proc_with_parent(10, Some(999), "orphan"),
            proc_with_parent(11, Some(10), "child"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows, &v), vec![10, 11]);
        assert_eq!(rows[0].depth, 0);
        assert_eq!(rows[1].depth, 1);
    }

    #[test]
    fn build_tree_view_self_parent_becomes_root() {
        let v = vec![proc_with_parent(5, Some(5), "x")];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].depth, 0);
    }

    #[test]
    fn build_tree_view_breaks_cycle() {
        let v = vec![
            proc_with_parent(1, Some(2), "a"),
            proc_with_parent(2, Some(1), "b"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        assert_eq!(rows.len(), 2);
        let roots = rows.iter().filter(|r| r.depth == 0).count();
        assert!(roots >= 1);
    }

    #[test]
    fn build_tree_view_collapsed_hides_descendants() {
        let v = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "child"),
            proc_with_parent(3, Some(2), "grand"),
        ];
        let mut collapsed = HashSet::new();
        collapsed.insert(1);
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &collapsed, "", None);
        assert_eq!(pids(&rows, &v), vec![1]);
        assert!(rows[0].is_collapsed);
        assert_eq!(rows[0].collapsed_count, 2);
    }

    #[test]
    fn build_tree_view_collapsed_leaf_is_not_marked() {
        let v = vec![proc_with_parent(1, None, "leaf")];
        let mut collapsed = HashSet::new();
        collapsed.insert(1);
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &collapsed, "", None);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].is_collapsed, "folha não deve aparecer como colapsada");
        assert!(!rows[0].has_children);
    }

    #[test]
    fn build_tree_view_filter_preserves_ancestor_chain() {
        let v = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "branch"),
            proc_with_parent(3, Some(2), "target"),
            proc_with_parent(4, Some(1), "noise"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "target", None);
        assert_eq!(pids(&rows, &v), vec![1, 2, 3]);
    }

    #[test]
    fn build_tree_view_filter_no_match_returns_empty() {
        let v = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "child"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "zzz", None);
        assert!(rows.is_empty());
    }

    #[test]
    fn build_tree_view_sorts_siblings_by_cpu_desc() {
        let v = vec![
            ProcessInfo { pid: 1, ppid: None, name: "p".into(), cpu: 0.0, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
            ProcessInfo { pid: 2, ppid: Some(1), name: "low".into(), cpu: 5.0, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
            ProcessInfo { pid: 3, ppid: Some(1), name: "high".into(), cpu: 90.0, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
            ProcessInfo { pid: 4, ppid: Some(1), name: "mid".into(), cpu: 50.0, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
        ];
        let rows = build_tree_view(&v, SortKey::Cpu, SortDir::Desc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows, &v), vec![1, 3, 4, 2]);
    }

    #[test]
    fn build_tree_view_branches_string_for_deep_tree() {
        // raiz com 2 filhos; primeiro filho tem 2 netos
        let v = vec![
            proc_with_parent(1, None, "a"),
            proc_with_parent(2, Some(1), "b"),
            proc_with_parent(3, Some(1), "c"),
            proc_with_parent(4, Some(2), "d"),
            proc_with_parent(5, Some(2), "e"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        // ordem DFS: 1, 2, 4, 5, 3
        assert_eq!(pids(&rows, &v), vec![1, 2, 4, 5, 3]);

        // 1: root, sem branches
        assert_eq!(rows[0].branches, vec![]);
        // 2: filho de 1, com irmão (3) depois → Tee
        assert_eq!(rows[1].branches, vec![BranchKind::Tee]);
        // 4: neto, pai 2 ainda tem irmão (3) abaixo → Pipe; 4 tem irmão (5) → Tee
        assert_eq!(rows[2].branches, vec![BranchKind::Pipe, BranchKind::Tee]);
        // 5: neto, pai 2 ainda tem irmão (3) → Pipe; 5 é último → Corner
        assert_eq!(rows[3].branches, vec![BranchKind::Pipe, BranchKind::Corner]);
        // 3: filho de 1, último → Corner
        assert_eq!(rows[4].branches, vec![BranchKind::Corner]);
    }

    #[test]
    fn toggle_view_mode_swaps_and_clears_collapsed() {
        let mut s = AppState::new();
        s.collapsed.insert(123);
        assert_eq!(s.view_mode, ViewMode::Flat);
        s.toggle_view_mode();
        assert_eq!(s.view_mode, ViewMode::Tree);
        assert!(s.collapsed.is_empty(), "alternar modo limpa colapsados");
        s.toggle_view_mode();
        assert_eq!(s.view_mode, ViewMode::Flat);
    }

    #[test]
    fn toggle_collapsed_inserts_and_removes() {
        let mut s = AppState::new();
        s.toggle_collapsed(42);
        assert!(s.collapsed.contains(&42));
        s.toggle_collapsed(42);
        assert!(!s.collapsed.contains(&42));
    }

    #[test]
    fn handle_key_t_toggles_view_mode() {
        let mut s = AppState::new();
        handle_key(&mut s, KeyCode::Char('t'), 0, 10, None);
        assert_eq!(s.view_mode, ViewMode::Tree);
        handle_key(&mut s, KeyCode::Char('T'), 0, 10, None);
        assert_eq!(s.view_mode, ViewMode::Flat);
    }

    #[test]
    fn handle_key_enter_in_tree_toggles_collapse_on_selected() {
        let mut s = AppState::new();
        s.view_mode = ViewMode::Tree;
        let sel = (777u32, "x".to_string());
        handle_key(&mut s, KeyCode::Enter, 1, 10, Some(&sel));
        assert!(s.collapsed.contains(&777));
        handle_key(&mut s, KeyCode::Enter, 1, 10, Some(&sel));
        assert!(!s.collapsed.contains(&777));
    }

    #[test]
    fn handle_key_enter_in_flat_is_noop() {
        let mut s = AppState::new();
        let sel = (1u32, "x".to_string());
        handle_key(&mut s, KeyCode::Enter, 1, 10, Some(&sel));
        assert!(s.collapsed.is_empty());
    }

    #[test]
    fn capture_tree_order_records_siblings_by_parent() {
        let v = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "a"),
            proc_with_parent(3, Some(1), "b"),
            proc_with_parent(4, Some(2), "grand"),
            proc_with_parent(5, None, "another_root"),
        ];
        let rows = build_tree_view(&v, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        let captured = capture_tree_order(&rows, &v);
        assert_eq!(captured.get(&None).unwrap(), &vec![1, 5]);
        assert_eq!(captured.get(&Some(1)).unwrap(), &vec![2, 3]);
        assert_eq!(captured.get(&Some(2)).unwrap(), &vec![4]);
    }

    #[test]
    fn build_tree_view_with_frozen_order_preserves_siblings_under_changing_cpu() {
        let make = |cpu2: f32, cpu3: f32| {
            vec![
                ProcessInfo { pid: 1, ppid: None, name: "root".into(), cpu: 0.0, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
                ProcessInfo { pid: 2, ppid: Some(1), name: "a".into(), cpu: cpu2, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
                ProcessInfo { pid: 3, ppid: Some(1), name: "b".into(), cpu: cpu3, memory_mb: 0.0, disk_read_mb: 0.0, disk_write_mb: 0.0 },
            ]
        };
        // ordena por CPU desc: pid 2 tem 90, pid 3 tem 10 → ordem 2,3
        let v1 = make(90.0, 10.0);
        let rows1 = build_tree_view(&v1, SortKey::Cpu, SortDir::Desc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows1, &v1), vec![1, 2, 3]);

        // captura ordem
        let frozen = capture_tree_order(&rows1, &v1);

        // CPU inverte (3 agora alto, 2 baixo) — sem frozen, ordem mudaria para 1,3,2
        let v2 = make(5.0, 95.0);
        let rows_unfrozen = build_tree_view(&v2, SortKey::Cpu, SortDir::Desc, &HashSet::new(), "", None);
        assert_eq!(pids(&rows_unfrozen, &v2), vec![1, 3, 2]);

        // com frozen, ordem permanece 1,2,3
        let rows_frozen = build_tree_view(&v2, SortKey::Cpu, SortDir::Desc, &HashSet::new(), "", Some(&frozen));
        assert_eq!(pids(&rows_frozen, &v2), vec![1, 2, 3]);
    }

    #[test]
    fn build_tree_view_with_frozen_appends_new_pids_sorted_at_end() {
        // Trava captura ordem [2, 3] como filhos de 1. Depois aparece o pid 4.
        let v_initial = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "a"),
            proc_with_parent(3, Some(1), "b"),
        ];
        let rows_initial = build_tree_view(&v_initial, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        let frozen = capture_tree_order(&rows_initial, &v_initial);

        let v_after = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "a"),
            proc_with_parent(3, Some(1), "b"),
            proc_with_parent(4, Some(1), "novo"),
        ];
        let rows = build_tree_view(&v_after, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", Some(&frozen));
        // 2 e 3 mantêm posição original; 4 aparece no fim
        assert_eq!(pids(&rows, &v_after), vec![1, 2, 3, 4]);
    }

    #[test]
    fn build_tree_view_with_frozen_drops_dead_pids_silently() {
        let v_initial = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "a"),
            proc_with_parent(3, Some(1), "b"),
            proc_with_parent(4, Some(1), "c"),
        ];
        let rows_initial = build_tree_view(&v_initial, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", None);
        let frozen = capture_tree_order(&rows_initial, &v_initial);

        // pid 3 morreu
        let v_after = vec![
            proc_with_parent(1, None, "root"),
            proc_with_parent(2, Some(1), "a"),
            proc_with_parent(4, Some(1), "c"),
        ];
        let rows = build_tree_view(&v_after, SortKey::Pid, SortDir::Asc, &HashSet::new(), "", Some(&frozen));
        assert_eq!(pids(&rows, &v_after), vec![1, 2, 4]);
    }

    #[test]
    fn toggle_freeze_off_clears_tree_order_too() {
        let mut s = AppState::new();
        s.frozen_tree_order.insert(None, vec![1, 2]);
        s.toggle_freeze();
        assert!(s.sort_frozen);
        s.toggle_freeze();
        assert!(!s.sort_frozen);
        assert!(s.frozen_tree_order.is_empty());
    }

    #[test]
    fn toggle_sort_invalidates_tree_order() {
        let mut s = AppState::new();
        s.toggle_freeze();
        s.frozen_tree_order.insert(None, vec![1, 2]);
        s.toggle_sort(SortKey::Cpu);
        assert!(s.frozen_tree_order.is_empty());
    }

    #[test]
    fn toggle_view_mode_clears_tree_order() {
        let mut s = AppState::new();
        s.frozen_tree_order.insert(None, vec![1, 2]);
        s.frozen_order = vec![1, 2];
        s.toggle_view_mode();
        assert!(s.frozen_tree_order.is_empty());
        assert!(s.frozen_order.is_empty());
    }

    #[test]
    fn handle_key_t_in_filter_edit_is_typed_not_toggle() {
        let mut s = AppState::new();
        s.start_filter_edit();
        handle_key(&mut s, KeyCode::Char('t'), 0, 10, None);
        assert_eq!(s.view_mode, ViewMode::Flat);
        assert_eq!(s.filter, "t");
    }
}
