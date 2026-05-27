use crate::app::{AppState, FilterMode, SortDir, SortKey};
use crate::process::{History, ProcessDetail, SystemSnapshot};
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Wrap},
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
        detail: Option<&ProcessDetail>,
        history: &History,
    ) -> io::Result<()> {
        self.terminal
            .draw(|frame| ui(frame, snapshot, visible, state, detail, history))?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

fn ui(
    frame: &mut Frame,
    snapshot: &SystemSnapshot,
    visible: &[usize],
    state: &mut AppState,
    detail: Option<&ProcessDetail>,
    history: &History,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header (title + hints)
            Constraint::Length(3), // gauges row (CPU | RAM)
            Constraint::Length(4), // sparklines row (CPU | RAM)
            Constraint::Min(0),    // process table
            Constraint::Length(1), // footer (filtro)
        ])
        .split(frame.area());

    let hint_style = Style::default().fg(Color::DarkGray);
    let mut title_spans = vec![
        Span::styled(
            "=== Monitorando 1.0 ===",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("⟳ {}", format_interval(state.refresh_ms)),
            Style::default().fg(Color::DarkGray),
        ),
    ];
    if state.paused {
        title_spans.push(Span::raw("   "));
        title_spans.push(Span::styled(
            "[PAUSADO]",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let header = Paragraph::new(vec![
        Line::from(title_spans),
        Line::from(vec![
            Span::styled("↑↓", hint_style),
            Span::raw(" navegar  "),
            Span::styled("c/m/n/p", hint_style),
            Span::raw(" ordenar  "),
            Span::styled("/", hint_style),
            Span::raw(" filtrar  "),
            Span::styled("d", hint_style),
            Span::raw(" detalhes  "),
            Span::styled("k", hint_style),
            Span::raw(" matar  "),
            Span::styled("space", hint_style),
            Span::raw(" pausa  "),
            Span::styled("+/-", hint_style),
            Span::raw(" vel  "),
            Span::styled("q", hint_style),
            Span::raw(" sair"),
        ]),
    ]);
    frame.render_widget(header, chunks[0]);

    let gauges = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let cpu_ratio = (snapshot.cpu_usage / 100.0).clamp(0.0, 1.0) as f64;
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU"))
        .gauge_style(Style::default().fg(cpu_color(snapshot.cpu_usage)))
        .ratio(cpu_ratio)
        .label(format!("{:.1}%", snapshot.cpu_usage));
    frame.render_widget(cpu_gauge, gauges[0]);

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
    frame.render_widget(ram_gauge, gauges[1]);

    let sparks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let span = format_history_span(state.refresh_ms, history.capacity());
    let cpu_data = history.cpu();
    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("CPU {span}")),
        )
        .data(&cpu_data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(cpu_spark, sparks[0]);

    let ram_data = history.ram_pct();
    let ram_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("RAM {span}")),
        )
        .data(&ram_data)
        .max(100)
        .style(Style::default().fg(Color::Magenta));
    frame.render_widget(ram_spark, sparks[1]);

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

    let footer = if let Some(msg) = state.status_msg.as_ref() {
        Paragraph::new(Line::from(Span::styled(
            msg.clone(),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )))
    } else {
        match state.filter_mode {
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
        }
    };
    frame.render_widget(footer, chunks[4]);

    if state.show_details {
        render_details_popup(frame, frame.area(), detail);
    }

    if let Some(prompt) = state.kill_prompt.as_ref() {
        render_kill_popup(frame, frame.area(), prompt.pid, &prompt.name);
    }
}

fn render_details_popup(frame: &mut Frame, area: Rect, detail: Option<&ProcessDetail>) {
    let popup = centered_rect_pct(75, 70, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Detalhes (d/Esc fecha) ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let body: Vec<Line> = if let Some(d) = detail {
        let label = Style::default().fg(Color::DarkGray);
        let value = Style::default().fg(Color::White);
        let dash = || "—".to_string();
        vec![
            row_line("Nome", d.name.clone(), label, value),
            row_line("PID", d.pid.to_string(), label, value),
            row_line(
                "PPID",
                d.ppid.map(|p| p.to_string()).unwrap_or_else(dash),
                label,
                value,
            ),
            row_line("Status", d.status.clone(), label, value),
            row_line("Tempo ativo", format_duration(d.run_time_secs), label, value),
            row_line("CPU", format!("{:.2}%", d.cpu), label, value),
            row_line(
                "Memória",
                format!(
                    "{:.1} MB (RSS)  ·  {:.1} MB (VIRT)",
                    d.memory_mb, d.virtual_memory_mb
                ),
                label,
                value,
            ),
            row_line(
                "Disco",
                format!(
                    "lido {:.1} MB  ·  escrito {:.1} MB",
                    d.disk_read_mb, d.disk_write_mb
                ),
                label,
                value,
            ),
            row_line("Executável", d.exe.clone().unwrap_or_else(dash), label, value),
            row_line("Diretório", d.cwd.clone().unwrap_or_else(dash), label, value),
            row_line(
                "Comando",
                if d.cmd.is_empty() { dash() } else { d.cmd.clone() },
                label,
                value,
            ),
        ]
    } else {
        vec![Line::from(Span::styled(
            "Processo selecionado não está mais disponível.",
            Style::default().fg(Color::Yellow),
        ))]
    };

    let para = Paragraph::new(body).block(block).wrap(Wrap { trim: false });
    frame.render_widget(para, popup);
}

fn row_line<'a>(
    label: &str,
    value: String,
    label_style: Style,
    value_style: Style,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<13}"), label_style),
        Span::styled(value, value_style),
    ])
}

fn render_kill_popup(frame: &mut Frame, area: Rect, pid: u32, name: &str) {
    let popup = centered_rect_pct(50, 25, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Confirmar ",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("Encerrar processo "),
            Span::styled(
                format!("PID {pid}"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ("),
            Span::styled(
                name.to_string(),
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::raw(")?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" confirmar    "),
            Span::styled("n / Esc", Style::default().fg(Color::DarkGray)),
            Span::raw(" cancelar"),
        ]),
    ];

    let para = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(para, popup);
}

fn centered_rect_pct(pct_x: u16, pct_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn format_interval(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        let secs = ms as f64 / 1000.0;
        format!("{secs:.1}s")
    }
}

fn format_history_span(refresh_ms: u64, capacity: usize) -> String {
    let total_secs = refresh_ms.saturating_mul(capacity as u64) / 1000;
    if total_secs < 60 {
        format!("{total_secs}s")
    } else {
        format!("{}min", total_secs / 60)
    }
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
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
    fn format_history_span_uses_seconds_then_minutes() {
        assert_eq!(format_history_span(250, 120), "30s");
        assert_eq!(format_history_span(500, 120), "1min");
        assert_eq!(format_history_span(1000, 120), "2min");
        assert_eq!(format_history_span(5000, 120), "10min");
    }

    #[test]
    fn format_interval_uses_ms_below_one_second() {
        assert_eq!(format_interval(250), "250ms");
        assert_eq!(format_interval(500), "500ms");
        assert_eq!(format_interval(1000), "1.0s");
        assert_eq!(format_interval(2000), "2.0s");
        assert_eq!(format_interval(5000), "5.0s");
    }

    #[test]
    fn format_duration_picks_largest_unit() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(60), "1m 0s");
        assert_eq!(format_duration(3 * 60 + 7), "3m 7s");
        assert_eq!(format_duration(3600), "1h 0m 0s");
        assert_eq!(format_duration(2 * 3600 + 5 * 60 + 9), "2h 5m 9s");
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
