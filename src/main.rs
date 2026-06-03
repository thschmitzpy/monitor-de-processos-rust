mod app;
mod config;
mod display;
mod process;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use std::io;
use std::time::Duration;

const PAGE_SIZE: usize = 10;

fn main() {
    if let Err(e) = run() {
        eprintln!("Erro: {e}");
        std::process::exit(1);
    }
    println!("Monitorando encerrado.");
}

fn run() -> io::Result<()> {
    let cfg = config::load();
    let mut guard = display::TerminalGuard::new()?;
    let mut collector = process::Collector::new();
    let info = collector.info().clone();
    let mut state = app::AppState::new();
    state.sort_key = cfg.sort_key;
    state.sort_dir = cfg.sort_dir;
    state.refresh_ms = cfg.refresh_ms;
    let mut history = process::History::new();
    let mut snapshot = collector.snapshot();
    history.push(&snapshot);

    loop {
        if !state.paused {
            snapshot = collector.snapshot();
            history.push(&snapshot);
        }
        let rows = match state.view_mode {
            app::ViewMode::Flat => {
                if state.sort_frozen && !state.frozen_order.is_empty() {
                    app::stable_reorder(&mut snapshot.processes, &mut state.frozen_order);
                } else {
                    app::sort_processes(&mut snapshot.processes, state.sort_key, state.sort_dir);
                    if state.sort_frozen {
                        state.frozen_order =
                            snapshot.processes.iter().map(|p| p.pid).collect();
                    }
                }
                let visible = app::filter_indices(&snapshot.processes, &state.filter);
                app::build_flat_view(&visible)
            }
            app::ViewMode::Tree => {
                let frozen_param = if state.sort_frozen && !state.frozen_tree_order.is_empty() {
                    Some(&state.frozen_tree_order)
                } else {
                    None
                };
                let tree_rows = app::build_tree_view(
                    &snapshot.processes,
                    state.sort_key,
                    state.sort_dir,
                    &state.collapsed,
                    &state.filter,
                    frozen_param,
                );
                if state.sort_frozen && state.frozen_tree_order.is_empty() {
                    state.frozen_tree_order =
                        app::capture_tree_order(&tree_rows, &snapshot.processes);
                }
                tree_rows
            }
        };
        state.clamp_to(rows.len());

        let selected_pid_name = selected_process(&snapshot, &rows, &state)
            .map(|p| (p.pid, p.name.clone()));

        let detail = if state.show_details {
            selected_pid_name
                .as_ref()
                .and_then(|(pid, _)| collector.detail(*pid))
        } else {
            None
        };

        guard.draw(&snapshot, &info, &rows, &mut state, detail.as_ref(), &history)?;

        if event::poll(Duration::from_millis(state.refresh_ms))? {
            if let Event::Key(KeyEvent {
                code,
                kind: KeyEventKind::Press,
                ..
            }) = event::read()?
            {
                let action = app::handle_key(
                    &mut state,
                    code,
                    rows.len(),
                    PAGE_SIZE,
                    selected_pid_name.as_ref(),
                );
                match action {
                    app::Action::None => {}
                    app::Action::Quit => break,
                    app::Action::KillConfirmed { pid, name } => {
                        let ok = collector.kill(pid);
                        state.status_msg = Some(if ok {
                            format!("PID {pid} ({name}) encerrado")
                        } else {
                            format!("Falha ao encerrar PID {pid} ({name}) — sem permissão?")
                        });
                    }
                }
            }
        }
    }

    let _ = config::save(&config::Config::from_state(&state));
    Ok(())
}

fn selected_process<'a>(
    snapshot: &'a process::SystemSnapshot,
    rows: &[app::TreeRow],
    state: &app::AppState,
) -> Option<&'a process::ProcessInfo> {
    let row = state.table.selected()?;
    let idx = rows.get(row)?.idx;
    snapshot.processes.get(idx)
}
