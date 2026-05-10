mod display;
mod process;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use std::time::Duration;

fn main() {
    let mut collector = process::Collector::new();

    display::setup_terminal().expect("Falha ao configurar terminal");

    loop {
        let snapshot = collector.snapshot();

        if let Err(e) = display::render(&snapshot) {
            display::teardown_terminal().ok();
            eprintln!("Erro ao renderizar: {e}");
            break;
        }

        if event::poll(Duration::from_millis(1000)).unwrap_or(false) {
            if let Ok(Event::Key(KeyEvent { code, .. })) = event::read() {
                if code == KeyCode::Char('q') || code == KeyCode::Char('Q') {
                    break;
                }
            }
        }
    }

    display::teardown_terminal().expect("Falha ao restaurar terminal");
    println!("Monitorando encerrado.");
}
