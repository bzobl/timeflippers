use crossterm_027 as crossterm;
use std::{
    cmp::max,
    error::Error,
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::bail;
use chrono::Local;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use futures_timer::Delay;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState},
};
use serde::{Deserialize, Serialize};
use std::collections::{
    hash_map::Entry::{Occupied, Vacant},
    HashMap,
};
use timeflippers::{
    timeflip::{Entry, Event as TimeEvent},
    view::DurationView,
    BluetoothSession, Config, Facet, TimeFlip,
};
use tokio::{fs, select};
use tui_textarea::{Input, Key, TextArea};

struct StatefulList<T> {
    state: ListState,
    items: Vec<T>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>, selection: Option<usize>) -> StatefulList<T> {
        let mut list_state = ListState::default();
        list_state.select(selection.or(Some(0)));
        StatefulList {
            state: list_state,
            items,
        }
    }

    fn next(&mut self) {
        if self.items.is_empty() {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.items.is_empty() {
            self.state.select(None);
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn remove(&mut self) {
        self.state.selected().map(|i| {
            self.items.remove(i);
            if self.items.is_empty() {
                self.state.select(None);
            } else if i >= self.items.len() {
                self.state.select(Some(self.items.len()));
            } else {
                self.state.select(Some(i));
            }
        });
    }

    fn selected(&self) -> Option<&T> {
        self.state.selected().map(|i| self.items.get(i)).flatten()
    }
}

struct App {
    items: StatefulList<u32>,
    entries: HashMap<u32, MyEntry>,
    show_invisible: bool,
}

impl App {
    fn new_from_entries(entries: Vec<MyEntry>) -> App {
        let map = entries.iter().map(|e| (e.entry.id, e.clone())).collect();
        let entry_ids = entries
            .iter()
            .filter_map(|e| {
                if e.visible && e.entry.duration > Duration::from_secs(30) {
                    Some(e.entry.id)
                } else {
                    None
                }
            })
            .collect();
        App {
            items: StatefulList::with_items(entry_ids, None),
            entries: map,
            show_invisible: false,
        }
    }

    fn update_entry_list(&mut self) {
        let mut new_items: Vec<u32> = self
            .entries
            .values()
            .filter_map(|e| {
                if e.entry.duration > Duration::from_secs(30) {
                    match (e.visible, self.show_invisible) {
                        (true, _) | (false, true) => Some(e.entry.id),
                        _ => None,
                    }
                } else {
                    None
                }
            })
            .collect();
        new_items.sort();
        let selection = self
            .items
            .selected()
            .map(|currently_selected| new_items.iter().position(|e| e == currently_selected))
            .flatten();
        self.items = StatefulList::with_items(new_items, selection);
    }

    fn toggle_visibility(&mut self) {
        self.show_invisible = !self.show_invisible;
        self.update_entry_list();
    }
}

#[derive(Parser)]
#[clap(about)]
struct Options {
    #[arg(help = "path to the timeflip.toml file")]
    config: PathBuf,
    #[arg(help = "read events from and write new events to file")]
    persistent_file: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Options::parse();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal, opt).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyEntry {
    #[serde(flatten)]
    entry: Entry,
    #[serde(default)]
    description: Vec<String>,
    #[serde(default)]
    visible: bool,
}

async fn load_history(persistent_file: &PathBuf) -> anyhow::Result<(u32, Vec<MyEntry>)> {
    match fs::read_to_string(persistent_file).await {
        Ok(s) => {
            let mut entries: Vec<MyEntry> = serde_json::from_str(&s)?;
            entries.sort_by(|a, b| a.entry.id.cmp(&b.entry.id));
            Ok((entries.last().map(|e| e.entry.id).unwrap_or(0), entries))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok((0, vec![])),
        Err(e) => Err(e.into()),
    }
}

async fn persist_history(persistent_file: &PathBuf, entries: &[MyEntry]) -> anyhow::Result<()> {
    match serde_json::to_vec(&entries) {
        Ok(json) => {
            if let Err(e) = fs::write(&persistent_file, json).await {
                bail!(
                    "cannot update entries file {}: {e}",
                    persistent_file.display()
                );
            }
        }
        Err(e) => bail!(
            "cannot update entries file {}: {e}",
            persistent_file.display()
        ),
    }
    Ok(())
}

async fn read_config(path: impl AsRef<Path>) -> anyhow::Result<Config> {
    let toml = fs::read_to_string(path).await?;
    let config: Config = toml::from_str(&toml)?;
    Ok(config)
}

fn facet_name(facet: &Facet, config: &Config) -> String {
    config.sides[facet.index_zero()]
        .name
        .clone()
        .unwrap_or(facet.to_string())
}

fn longest_facet_name(config: &Config) -> usize {
    config
        .sides
        .iter()
        .map(|side| side.name.clone().unwrap_or(side.facet.to_string()).len())
        .max()
        .unwrap_or_default()
}

enum State {
    Selecting,
    Editing,
    Paused,
}

impl State {
    fn get_description(&self) -> String {
        match self {
            Self::Selecting => {
                String::from("[Up/Down] Move, [->] Edit, [p] Pause, [d] Done, [t] Toggle Visibility, [s] Sync, [q] Quit")
            }
            Self::Editing => String::from("[Esc] Finish editing"),
            Self::Paused => String::from("[p] Unpause"),
        }
    }
}

async fn run<B: Backend>(terminal: &mut Terminal<B>, opt: Options) -> anyhow::Result<()> {
    let config = read_config(opt.config).await?;
    let (mut last_seen, entries) = load_history(&opt.persistent_file).await?;

    let (mut bg_task, session) = BluetoothSession::new().await?;
    let timeflip = TimeFlip::connect(&session, Some(config.password)).await?;

    let mut app = App::new_from_entries(entries);

    let update: Vec<Entry> = timeflip
        .read_history_since(last_seen)
        .await?
        .into_iter()
        .collect();
    for entry in update {
        last_seen = max(entry.id, last_seen);
        match app.entries.entry(entry.id) {
            Vacant(v) => {
                v.insert(MyEntry {
                    entry,
                    description: vec![],
                    visible: true,
                });
            }
            Occupied(mut o) => {
                o.get_mut().entry = entry;
            }
        }
    }
    app.update_entry_list();

    let mut textarea = if let Some(selected) = &app.items.selected() {
        let text = app
            .entries
            .get(selected)
            .expect("must be present")
            .description
            .clone();
        TextArea::new(text.to_vec())
    } else {
        TextArea::default()
    };

    let mut state = State::Selecting;
    let mut reader = EventStream::new();

    timeflip.subscribe_double_tap().await?;
    timeflip.subscribe_facet().await?;
    let mut stream = timeflip.event_stream().await?;

    loop {
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title("Additional information"),
        );
        terminal.draw(|f| ui(f, &mut app, &textarea, &state, &config))?;

        let delay = Delay::new(Duration::from_millis(1_000));
        select! {
            event = stream.next() => {
                match event {
                    Some(TimeEvent::DoubleTap { pause, .. }) => {
                        match state {
                            State::Paused => {
                                if !pause {
                                    state = State::Selecting;
                                }
                            }
                            _ => {
                                if pause {
                                    state = State::Paused;
                                }
                            }
                        }
                    },
                    Some(TimeEvent::Facet(_facet)) => {
                        if matches!(state, State::Paused) {
                            state = State::Selecting;
                        }
                    }
                    Some(_) => continue,
                    None => continue,
                }
            }
            _ = delay => { continue; }
            res = &mut bg_task => {
                if let Err(e) =res {
                    bail!("bluetooth session background task exited with error: {e}");
                }
            }
            maybe_event = reader.next() => {
                if let Some(Ok(event)) = maybe_event {
                    match state {
                        State::Selecting => {
                        if let Event::Key(key) = event {
                            if key.kind == KeyEventKind::Press {
                                match key.code {
                                    KeyCode::Char('q') => {
                                        let entries: Vec<MyEntry> = app.entries.into_values().collect();
                                        persist_history(&opt.persistent_file, &entries).await?;
                                        return Ok(())
                                    },
                                    KeyCode::Char('p') => {
                                        timeflip.pause().await?;
                                        state = State::Paused;
                                    }
                                    KeyCode::Char('d') => {
                                        if let Some(selected) = app.items.selected() {
                                            let entry = app.entries.get_mut(selected).expect("must be present");
                                            entry.visible = !entry.visible;
                                            if !entry.visible && !app.show_invisible {
                                              app.items.remove();
                                            }
                                        }
                                    }
                                    KeyCode::Char('t') => {
                                        app.toggle_visibility();
                                    }
                                    KeyCode::Char('s') => {
                                      let update: Vec<Entry> = timeflip
                                          .read_history_since(last_seen)
                                          .await?
                                          .into_iter()
                                          .collect();
                                      for entry in update {
                                          last_seen = max(entry.id, last_seen);
                                          match app.entries.entry(entry.id) {
                                              Vacant(v) => {
                                                  v.insert(MyEntry {
                                                      entry,
                                                      description: vec![],
                                                      visible: true,
                                                  });
                                              }
                                              Occupied(mut o) => {
                                                  o.get_mut().entry = entry;
                                              }
                                          }
                                      }
                                      app.update_entry_list();
                                    }
                                    KeyCode::Right => {
                                        if app.items.selected().is_some() {
                                            state = State::Editing;
                                            textarea.set_style(Style::default().fg(Color::White));
                                        }
                                    }
                                    KeyCode::Down => {
                                        app.items.next();
                                    },
                                    KeyCode::Up => {
                                        app.items.previous();
                                    },
                                    _ => {
                                    }
                                }
                                let text = if let Some(selected) = app.items.selected() {
                                    app.entries.get(selected).expect("must be present").description.to_vec()
                                } else { vec!["".to_string()] };
                                textarea = TextArea::new(text);
                            }
                        }
                    },
                    State::Editing => {
                        match event.into() {
                            Input { key: Key::Esc, .. } => {
                                state = State::Selecting;
                                if let Some(editing_entry) = app.items.selected() {
                                    let entry = app.entries.get_mut(editing_entry).expect("must be present");
                                    entry.description = textarea.lines().to_vec();
                                }
                                textarea.set_style(Style::default().fg(Color::Gray));
                            },
                            input => {
                                textarea.input(input);
                            }
                        }
                    }
                    State::Paused => {
                        match event.into() {
                                Input { key: Key::Char('p'), .. } => {
                                    timeflip.unpause().await?;
                                    state = State::Selecting;
                                }
                                _ => {},
                        }
                    }
                }
            }
            }
        };
    }
}

fn ui<B: Backend>(
    f: &mut Frame<B>,
    app: &mut App,
    textarea: &TextArea,
    state: &State,
    config: &Config,
) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.size());
    f.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .title(state.get_description()),
        main_layout[1],
    );
    let inner_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_layout[0]);
    let list_selected_color = match state {
        State::Selecting => Color::White,
        State::Editing | State::Paused => Color::Gray,
    };
    let max_len = longest_facet_name(config);
    let items: Vec<ListItem> = app
        .items
        .items
        .iter()
        .map(|i| {
            let entry = app.entries.get(i).expect("integrity broken");
            let spaced_facet = format!(
                "{:width$}",
                facet_name(&entry.entry.facet, config),
                width = max_len
            );
            let end_time = entry.entry.time
                + chrono::Duration::from_std(entry.entry.duration).expect("should work");
            let local = Local::now().timezone();
            let additional_info = if app.show_invisible {
                format!(" [{}]", if entry.visible { "*" } else { " " })
            } else {
                "".to_string()
            };
            let line_text = format!(
                "{} {}  {}  {}-{}{}",
                spaced_facet,
                entry.entry.time.with_timezone(&local).format("%d.%m"),
                DurationView(&entry.entry.duration),
                entry.entry.time.with_timezone(&local).format("%H:%M"),
                end_time.with_timezone(&local).format("%H:%M"),
                additional_info
            );
            let lines = Line::from(line_text);
            ListItem::new(lines).style(Style::default().fg(list_selected_color).bg(Color::Black))
        })
        .collect();
    let items = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Timeflip entries"),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(list_selected_color)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(items, inner_layout[0], &mut app.items.state);
    f.render_widget(textarea.widget(), inner_layout[1]);
}
