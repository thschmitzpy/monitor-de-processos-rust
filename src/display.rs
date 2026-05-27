use crate::app::{AppState, FilterMode, SortDir, SortKey};
use crate::process::SystemSnapshot;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::io::{self, Stdout};

pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn draw(
        &mut self,
        snapshot: &SystemSnapshot,
        visible: &[usize],
        state: &mut AppState,
    ) -> io::Result<()> {
        self.terminal
            .draw(|frame| ui(frame, snapshot, visible, state))?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

fn ui(frame: &mut Frame, snapshot: &SystemSnapshot, visible: &[usize], state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header (title + hints)
            Constraint::Length(3), // CPU gauge
            Constraint::Length(3), // RAM gauge
            Constraint::Min(0),    // process table
            Constraint::Length(1), // footer (filtro)
        ])
        .split(frame.area());

    let hint_style = Style::default().fg(Color::DarkGray);
    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "=== Monitorando 1.0 ===",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("↑↓ PgUp/PgDn Home/End", hint_style),
            Span::raw(" navegar  "),
            Span::styled("c/m/n/p", hint_style),
            Span::raw(" ordenar  "),
            Span::styled("/", hint_style),
            Span::raw(" filtrar  "),
            Span::styled("q", hint_style),
            Span::raw(" sair"),
        ]),
    ]);
    frame.render_widget(header, chunks[0]);

    let cpu_ratio = (snapshot.cpu_usage / 100.0).clamp(0.0, 1.0) as f64;
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .gauge_style(Style::default().fg(cpu_color(snapshot.cpu_usage)))
        .ratio(cpu_ratio)
        .label(format!("{:.1}%", snapshot.cpu_usage));
    frame.render_widget(cpu_gauge, chunks[1]);

    let ram_ratio = if snapshot.total_memory_gb > 0.0 {
        (snapshot.used_memory_gb / snapshot.total_memory_gb).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let ram_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("RAM"))
        .gauge_style(Style::default().fg(mem_color((ram_ratio * 100.0) as f32)))
        .ratio(ram_ratio)
        .label(format!(
            "{:.1} / {:.1} GB",
            snapshot.used_memory_gb, snapshot.total_memory_gb
        ));
    frame.render_widget(ram_gauge, chunks[2]);

    let header_row = Row::new(vec![
        Cell::from(sort_label("PID", SortKey::Pid, state)),
        Cell::from(sort_label("NOME", SortKey::Name, state)),
        Cell::from(sort_label("CPU%", SortKey::Cpu, state)),
        Cell::from(sort_label("MEM (MB)", SortKey::Memory, state)),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let rows = visible.iter().filter_map(|&i| {
        let p = snapshot.processes.get(i)?;
        Some(
            Row::new(vec![
                Cell::from(p.pid.to_string()),
                Cell::from(p.name.clone()),
                Cell::from(format!("{:.2}", p.cpu)),
                Cell::from(format!("{:.1}", p.memory_mb)),
            ])
            .style(Style::default().fg(process_cpu_color(p.cpu))),
        )
    });

    let widths = [
        Constraint::Length(7),
        Constraint::Min(20),
        Constraint::Length(8),
        Constraint::Length(12),
    ];
    let title = if state.filter.is_empty() {
        format!("Processos ({})", snapshot.processes.len())
    } else {
        format!(
            "Processos ({}/{})",
            visible.len(),
            snapshot.processes.len()
        )
    };
    let table = Table::new(rows, widths)
        .header(header_row)
        .block(Block::default().borders(Borders::ALL).title(title))
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(table, chunks[3], &mut state.table);

    let footer = match state.filter_mode {
        FilterMode::Inactive => Paragraph::new(Line::raw("")),
        FilterMode::Editing => Paragraph::new(Line::from(vec![
            Span::styled(
                "/ ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(state.filter.clone()),
            Span::styled("_", Style::default().fg(Color::Cyan)),
            Span::raw("    "),
            Span::styled(
                "Enter aplica  Esc cancela",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        FilterMode::Applied => Paragraph::new(Line::from(vec![
            Span::styled("filtro: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                state.filter.clone(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                "Esc limpa  / edita",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
    };
    frame.render_widget(footer, chunks[4]);
}

fn sort_label(label: &str, col: SortKey, state: &AppState) -> String {
    if state.sort_key == col {
        let arrow = match state.sort_dir {
            SortDir::Asc => '▲',
            SortDir::Desc => '▼',
        };
        format!("{label} {arrow}")
    } else {
        label.to_string()
    }
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

fn process_cpu_color(cpu: f32) -> Color {
    if cpu >= 50.0 {
        Color::Red
    } else if cpu >= 10.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn mem_color(pct: f32) -> Color {
    if pct >= 85.0 {
        Color::Red
    } else if pct >= 60.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_color_thresholds() {
        assert!(matches!(cpu_color(0.0), Color::Green));
        assert!(matches!(cpu_color(4.99), Color::Green));
        assert!(matches!(cpu_color(5.0), Color::Yellow));
        assert!(matches!(cpu_color(19.99), Color::Yellow));
        assert!(matches!(cpu_color(20.0), Color::Red));
        assert!(matches!(cpu_color(100.0), Color::Red));
    }

    #[test]
    fn process_cpu_color_thresholds() {
        assert!(matches!(process_cpu_color(0.0), Color::Green));
        assert!(matches!(process_cpu_color(9.99), Color::Green));
        assert!(matches!(process_cpu_color(10.0), Color::Yellow));
        assert!(matches!(process_cpu_color(49.99), Color::Yellow));
        assert!(matches!(process_cpu_color(50.0), Color::Red));
        assert!(matches!(process_cpu_color(750.0), Color::Red));
    }

    #[test]
    fn mem_color_thresholds() {
        assert!(matches!(mem_color(0.0), Color::Green));
        assert!(matches!(mem_color(59.99), Color::Green));
        assert!(matches!(mem_color(60.0), Color::Yellow));
        assert!(matches!(mem_color(84.99), Color::Yellow));
        assert!(matches!(mem_color(85.0), Color::Red));
        assert!(matches!(mem_color(100.0), Color::Red));
    }
}
