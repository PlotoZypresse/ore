use crossterm::event::{
    self, Event,
    KeyCode::{self},
    KeyEvent, KeyEventKind,
};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    symbols::{
        self,
        border::{self},
    },
    text::{Line, Text},
    widgets::{Block, Gauge, Paragraph, Row, Table, Tabs, Widget},
};
use std::{collections::HashMap, ffi::OsString, io, time::Duration, vec};
use sysinfo::{Disks, Pid, System};

const BYTES_IN_GB: f64 = 1073741824.0;

#[derive(Debug)]
struct ProcessesEntry {
    cpu_usage: f32,
    memory_usage: u64,
    pid: Pid,
    name: OsString,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
enum SortKey {
    #[default]
    Memory,
    CPU,
}

#[derive(Default, Debug)]
struct Processes {
    entry: Vec<ProcessesEntry>,
    sort: SortKey,
}
impl Processes {
    fn new(sys: &System) -> Self {
        let mut processes = Self {
            entry: Vec::new(),
            sort: SortKey::Memory,
        };
        processes.fill_processes(sys);
        processes
    }

    fn fill_processes(&mut self, sys: &System) {
        self.entry.clear();

        for (pid, process) in sys.processes() {
            let entry = ProcessesEntry {
                cpu_usage: process.cpu_usage(),
                memory_usage: process.memory(),
                pid: *pid,
                name: process.name().to_os_string(),
            };
            self.entry.push(entry);
        }
    }

    fn set_sort_key(&mut self, key: SortKey) {
        self.sort = match key {
            SortKey::Memory => SortKey::Memory,
            SortKey::CPU => SortKey::CPU,
        };
        self.sort_processes();
    }

    fn sort_processes(&mut self) {
        match self.sort {
            SortKey::Memory => self.entry.sort_unstable_by(|a, b| {
                b.memory_usage
                    .cmp(&a.memory_usage)
                    .then_with(|| a.pid.cmp(&b.pid))
            }),
            SortKey::CPU => self.entry.sort_unstable_by(|a, b| {
                b.cpu_usage
                    .total_cmp(&a.cpu_usage)
                    .then_with(|| a.pid.cmp(&b.pid))
            }),
        }
    }
}

#[derive(Debug, Default)]
struct Storage {
    total_storage: u64,
    available_storage: u64,
}

impl Storage {
    fn new() -> Self {
        Self {
            total_storage: Self::get_total_storage(),
            available_storage: Self::get_available_storage(),
        }
    }

    fn get_total_storage() -> u64 {
        let disks = Disks::new_with_refreshed_list();

        // creates a hashmap as macos reports double the size as disks are reported multiple times.
        let mut seen_disks: HashMap<OsString, u64> = HashMap::new();
        for disk in disks.list() {
            seen_disks
                .entry(disk.name().to_os_string())
                .or_insert(disk.total_space());
        }
        return seen_disks.values().sum();
    }

    fn get_available_storage() -> u64 {
        let disks = Disks::new_with_refreshed_list();

        // creates a hashmap as macos reports double the size as disks are reported multiple times.
        let mut seen_disks: HashMap<OsString, u64> = HashMap::new();
        for disk in disks.list() {
            seen_disks
                .entry(disk.name().to_os_string())
                .or_insert(disk.available_space());
        }
        return seen_disks.values().sum();
    }
}

#[derive(Debug, Default)]
struct Memory {
    total_mem: f64,
    used_mem: f64,
    total_swap: f64,
    used_swap: f64,
}

#[derive(Debug, Default)]
struct CPU {
    nr_cores: usize,
    cpu_usage: f32,
    per_core_usage: Vec<f32>,
}

impl CPU {
    fn new(sys: &System) -> Self {
        Self {
            nr_cores: sys.cpus().len(),
            cpu_usage: sys.global_cpu_usage(),
            per_core_usage: vec![0.0; sys.cpus().len()],
        }
    }

    fn per_core_usage(&mut self, sys: &System) {
        let mut cpu_core_nr = 0;
        for cpu in sys.cpus() {
            self.per_core_usage[cpu_core_nr] = cpu.cpu_usage();
            cpu_core_nr += 1;
        }
    }

    fn upadte_cpu_usage(&mut self, sys: &System) {
        self.cpu_usage = sys.global_cpu_usage()
    }
}

#[derive(Debug, Default)]
struct App {
    sys: System,
    memory: Memory,
    cpu: CPU,
    storage: Storage,
    sys_name: String,
    processes: Processes,
    selected_tab: usize,
    exit: bool,
}

impl App {
    fn new() -> Self {
        let sys = System::new_all();

        Self {
            memory: get_mem_usage(&sys),
            cpu: CPU::new(&sys),
            storage: Storage::new(),
            sys_name: get_sys_name(),
            selected_tab: 0,
            processes: Processes::new(&sys),
            sys,
            exit: false,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            self.upadte();
            terminal.draw(|frame| self.draw(frame))?;
            if event::poll(Duration::from_millis(500))? {
                self.handle_events()?;
            }
        }
        Ok(())
    }

    fn upadte(&mut self) {
        self.sys.refresh_all();
        self.memory = get_mem_usage(&self.sys);
        self.cpu.upadte_cpu_usage(&self.sys);
        self.cpu.per_core_usage(&self.sys);
        self.processes.fill_processes(&self.sys);
        let sort_key = match self.selected_tab {
            1 => SortKey::CPU,
            _ => SortKey::Memory,
        };
        self.processes.set_sort_key(sort_key);
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Tab => self.tab_switch(),
            _ => {}
        }
    }

    fn tab_switch(&mut self) {
        if self.selected_tab >= 1 {
            self.selected_tab = 0
        } else {
            self.selected_tab += 1
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn render_memory(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(Line::from(" Memory ".bold()).centered())
            .border_set(border::THICK);

        let inner = block.inner(area);
        block.render(area, buf);

        let [text_area, gauge_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

        let text = Text::from(vec![
            Line::from(format!(
                "Memory usage: {:.1}GB / {:.1}GB",
                self.memory.used_mem, self.memory.total_mem,
            )),
            Line::from(format!(
                "Swap usage: {:.1}GB / {:.1}GB",
                self.memory.used_swap, self.memory.total_swap
            )),
        ]);

        Paragraph::new(text).centered().render(text_area, buf);

        let used_mem_percent = if self.memory.total_mem > 0.0 {
            self.memory.used_mem / self.memory.total_mem
        } else {
            0.0
        };

        Gauge::default()
            .style(Modifier::BOLD)
            .gauge_style(Style::new().white().on_black())
            .label(format!("{:.1}% used", used_mem_percent * 100.0))
            .ratio(used_mem_percent)
            .render(gauge_area, buf);
    }

    fn render_cpu(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title(Line::from(" CPU ".bold()).centered())
            .border_set(border::THICK);

        let inner = block.inner(area);
        block.render(area, buf);

        let [nr_cores_area, usage_area] =
            Layout::vertical([Constraint::Length(2), Constraint::Min(0)]).areas(inner);

        let text = Text::from(vec![
            Line::from(format!("Number of CPU cores: {}", self.cpu.nr_cores)).centered(),
        ]);

        Paragraph::new(text).centered().render(nr_cores_area, buf);

        let mut rows = Vec::new();

        for core in (0..self.cpu.nr_cores).step_by(2) {
            if core + 1 < self.cpu.nr_cores {
                rows.push(Row::new(vec![
                    Line::from(format!(
                        "Core {:>2}: {:4.1}",
                        core, self.cpu.per_core_usage[core]
                    ))
                    .centered(),
                    Line::from(format!(
                        "Core {:>2}: {:4.1}",
                        core + 1,
                        self.cpu.per_core_usage[core + 1]
                    ))
                    .centered(),
                ]));
            } else {
                rows.push(Row::new(vec![
                    Line::from(format!(
                        "Core {:>2}: {:4.1}",
                        core, self.cpu.per_core_usage[core]
                    ))
                    .centered(),
                ]));
            }
        }

        let widths = [Constraint::Percentage(50), Constraint::Percentage(50)];

        Table::new(rows, widths).render(usage_area, buf);
    }

    fn render_processes(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().border_set(border::THICK);

        let row_header = Row::new(vec!["Name", "PID", "CPU (%)", "Memory (GB)"]).bold();
        let rows = self.processes.entry.iter().map(|process| {
            Row::new(vec![
                process.name.to_string_lossy().into_owned(),
                process.pid.to_string(),
                format!("{:.1}", process.cpu_usage),
                format!("{:.1}", process.memory_usage as f64 / BYTES_IN_GB),
            ])
        });

        let widths = [
            Constraint::Percentage(50), // Process Name gets half the screen
            Constraint::Percentage(15), // PID
            Constraint::Percentage(15), // CPU
            Constraint::Percentage(20), // Memory gets the remaining space
        ];

        let table = Table::new(rows, widths).header(row_header).block(block); //
        table.render(area, buf);
    }

    fn render_tabs(&self, area: Rect, buf: &mut Buffer) {
        let tabs = Tabs::new(vec!["Memory Usage", "CPU Usage"])
            .style(Style::default().white())
            .highlight_style(Style::default().magenta().on_black().bold())
            .select(self.selected_tab)
            .divider(symbols::DOT)
            .padding(" ", " ");

        tabs.render(area, buf);
    }

    fn render_disks(&self, area: Rect, buf: &mut Buffer) {
        // The outer block of the disk usage tile
        let block = Block::bordered()
            .title(Line::from(" Disk Usage ".bold()).centered())
            .border_set(border::THICK);

        let inner = block.inner(area);
        block.render(area, buf);

        let [text_area, gauge_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);

        let used_storage_percentage = if self.storage.total_storage > 0 {
            (self.storage.total_storage - self.storage.available_storage) as f64
                / self.storage.total_storage as f64
        } else {
            0.0
        };

        let total_storage = Text::from(vec![
            Line::from(format!(
                "Total system storage: {:.1}GB",
                (self.storage.total_storage as f64 / 1_000_000_000.0)
            )),
            Line::from(format!(
                "Used storage: {:.1}GB",
                ((self.storage.total_storage - self.storage.available_storage) as f64
                    / 1_000_000_000.0)
            )),
        ]);

        Paragraph::new(total_storage)
            .centered()
            .render(text_area, buf);

        Gauge::default()
            .style(Modifier::BOLD)
            .gauge_style(Style::new().white().on_black())
            .label(format!("{:.1}% used", used_storage_percentage * 100.0))
            .ratio(used_storage_percentage)
            .render(gauge_area, buf);
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // creates the outer "bounding block"
        let outer = Block::bordered()
            .title(Line::from(format!(" {}'s resource usage ", self.sys_name).bold()).centered())
            .title_bottom(Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]).centered())
            .border_set(border::THICK);

        // creates the are inside the outer border
        let inner = outer.inner(area);
        // renders the outer border
        outer.render(area, buf);

        // splits the inner area into a top and bottom part
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);

        //splits the top part (by using .areas(top)) into the mem_are and cpu_are
        let [mem_area, cpu_area] =
            Layout::horizontal([Constraint::Length(35), Constraint::Min(0)]).areas(top);

        let [mem_u, mem_l] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Min(0)]).areas(mem_area);

        // calls the cpu and mem render function and specifies the are in which they are rendered
        self.render_memory(mem_u, buf);
        self.render_cpu(cpu_area, buf);
        self.render_disks(mem_l, buf);
        self.render_processes(bottom, buf);
        self.render_tabs(bottom, buf);
    }
}

fn main() {
    let _ = ratatui::run(|terminal| App::new().run(terminal));
}

fn get_mem_usage(sys: &System) -> Memory {
    let total_mem = (sys.total_memory() as f64) / BYTES_IN_GB;
    let used_mem = (sys.used_memory() as f64) / BYTES_IN_GB;
    let total_swap = (sys.total_swap() as f64) / BYTES_IN_GB;
    let used_swap = (sys.used_swap() as f64) / BYTES_IN_GB;

    Memory {
        total_mem,
        used_mem,
        total_swap,
        used_swap,
    }
}

fn get_sys_name() -> String {
    match System::name() {
        Some(_) => "Your MacBook".to_string(),
        None => "Your Machine".to_string(),
    }
}
