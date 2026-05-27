mod app;
mod display;
mod process;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
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

    loop {
        let mut snapshot = collector.snapshot();
        app::sort_processes(&mut snapshot.processes, state.sort_key, state.sort_dir);
        let visible = app::filter_indices(&snapshot.processes, &state.filter);
        state.clamp_to(visible.len());
        guard.draw(&snapshot, &visible, &mut state)?;

        if event::poll(Duration::from_millis(1000))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if state.filter_mode == app::FilterMode::Editing {
                    match code {
                        KeyCode::Esc => state.clear_filter(),
                        KeyCode::Enter => state.confirm_filter(),
                        KeyCode::Backspace => {
                            state.filter.pop();
                        }
                        KeyCode::Char(ch) => state.filter.push(ch),
                        _ => {}
                    }
                } else {
                    let total = visible.len();
                    match code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        KeyCode::Char('/') => state.start_filter_edit(),
                        KeyCode::Esc => state.clear_filter(),
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            state.toggle_sort(app::SortKey::Cpu)
                        }
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            state.toggle_sort(app::SortKey::Memory)
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            state.toggle_sort(app::SortKey::Name)
                        }
                        KeyCode::Char('p') | KeyCode::Char('P') => {
                            state.toggle_sort(app::SortKey::Pid)
                        }
                        KeyCode::Down => state.select_next(total),
                        KeyCode::Up => state.select_prev(total),
                        KeyCode::PageDown => state.page_down(total, PAGE_SIZE),
                        KeyCode::PageUp => state.page_up(total, PAGE_SIZE),
                        KeyCode::Home => state.select_first(total),
                        KeyCode::End => state.select_last(total),
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}
