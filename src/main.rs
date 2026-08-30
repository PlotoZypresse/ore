use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};
use std::{default, fmt::format, io, mem, time::Duration};
use sysinfo::{Components, System};

const BYTES_IN_GB: f64 = 1073741824.0;

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
}

impl CPU {
    fn new(sys: &System) -> Self {
        Self {
            nr_cores: sys.cpus().len(),
            cpu_usage: sys.global_cpu_usage(),
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
    sys_name: String,
    exit: bool,
}

impl App {
    fn new() -> Self {
        let mut sys = System::new_all();
        Self {
            memory: get_mem_usage(&sys),
            cpu: CPU::new(&sys),
            sys_name: get_sys_name(),
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
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let titel = Line::from(format!(" {}'s resource usage ", self.sys_name).bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);

        let block = Block::bordered()
            .title(titel.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let memory_text = Text::from(vec![
            Line::from(format!(
                "Memory usage: {:.1}GB / {:.1}GB",
                self.memory.used_mem, self.memory.total_mem,
            )),
            Line::from(format!("Number of CPU cores: {}", self.cpu.nr_cores)),
            Line::from(format!("CPU usage: {:.1}%", self.cpu.cpu_usage)),
        ]);

        Paragraph::new(memory_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}

fn main() {
    // let mut sys = System::new_all();
    // let cpuusage = sys.cpus().len

    ratatui::run(|terminal| App::new().run(terminal));
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
        Some(sys_name) => "Your MacBook".to_string(),
        None => "Your Machine".to_string(),
    }
}
