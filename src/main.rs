mod app;
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
    let mut guard = display::TerminalGuard::new()?;
    let mut collector = process::Collector::new();
    let mut state = app::AppState::new();
    let mut history = process::History::new();
    let mut snapshot = collector.snapshot();
    history.push(&snapshot);

    loop {
        if !state.paused {
            snapshot = collector.snapshot();
            history.push(&snapshot);
        }
        app::sort_processes(&mut snapshot.processes, state.sort_key, state.sort_dir);
        let visible = app::filter_indices(&snapshot.processes, &state.filter);
        state.clamp_to(visible.len());

        let selected_pid_name = selected_process(&snapshot, &visible, &state)
            .map(|p| (p.pid, p.name.clone()));

        let detail = if state.show_details {
            selected_pid_name
                .as_ref()
                .and_then(|(pid, _)| collector.detail(*pid))
        } else {
            None
        };

        guard.draw(&snapshot, &visible, &mut state, detail.as_ref(), &history)?;

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
                    visible.len(),
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

    Ok(())
}

fn selected_process<'a>(
    snapshot: &'a process::SystemSnapshot,
    visible: &[usize],
    state: &app::AppState,
) -> Option<&'a process::ProcessInfo> {
    let row = state.table.selected()?;
    let idx = *visible.get(row)?;
    snapshot.processes.get(idx)
}
