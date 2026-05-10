use crate::process::SystemSnapshot;
use crossterm::{
    cursor,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, ClearType},
    QueueableCommand,
};
use std::io::{self, Write};

const MAX_ROWS: usize = 30;

pub fn render(snapshot: &SystemSnapshot) -> io::Result<()> {
    let mut stdout = io::stdout();

    stdout.queue(cursor::MoveTo(0, 0))?;
    stdout.queue(terminal::Clear(ClearType::FromCursorDown))?;

    // Header
    stdout.queue(SetForegroundColor(Color::Cyan))?;
    stdout.queue(Print("=== Monitorando 1.0 ==="))?;
    stdout.queue(ResetColor)?;
    stdout.queue(Print("  [q] sair\n"))?;

    // System stats
    stdout.queue(Print("CPU Total: "))?;
    stdout.queue(SetForegroundColor(cpu_color(snapshot.cpu_usage)))?;
    stdout.queue(Print(format!("{:5.1}%", snapshot.cpu_usage)))?;
    stdout.queue(ResetColor)?;
    stdout.queue(Print(format!(
        "   RAM: {:.1} GB / {:.1} GB\n\n",
        snapshot.used_memory_gb, snapshot.total_memory_gb
    )))?;

    // Table header
    stdout.queue(SetForegroundColor(Color::Yellow))?;
    stdout.queue(Print(format!(
        " {:>6}  {:<25} {:>6}  {:>10}\n",
        "PID", "NOME", "CPU%", "MEM (MB)"
    )))?;
    stdout.queue(Print(format!("{}\n", "─".repeat(54))))?;
    stdout.queue(ResetColor)?;

    // Process rows
    for proc in snapshot.processes.iter().take(MAX_ROWS) {
        let color = cpu_color(proc.cpu);
        stdout.queue(SetForegroundColor(color))?;
        stdout.queue(Print(format!(
            " {:>6}  {:<25} {:>6.2}  {:>10.1}\n",
            proc.pid,
            truncate(&proc.name, 25),
            proc.cpu,
            proc.memory_mb,
        )))?;
        stdout.queue(ResetColor)?;
    }

    stdout.flush()
}

fn cpu_color(cpu: f32) -> Color {
    if cpu >= 20.0 {
        Color::Red
    } else if cpu >= 5.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

pub fn setup_terminal() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.queue(cursor::Hide)?;
    stdout.queue(terminal::Clear(ClearType::All))?;
    stdout.flush()
}

pub fn teardown_terminal() -> io::Result<()> {
    terminal::disable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.queue(cursor::Show)?;
    stdout.queue(cursor::MoveTo(0, 0))?;
    stdout.queue(terminal::Clear(ClearType::All))?;
    stdout.flush()
}
