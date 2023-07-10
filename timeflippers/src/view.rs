use chrono::{DateTime, Local, TimeZone, Utc};
use std::{fmt, time::Duration};

use crate::config::Config;
use crate::timeflip::Entry;

mod table;
use table::{Position, TableHeader};

struct DurationView<'a>(&'a Duration);

impl<'a> fmt::Display for DurationView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let seconds = self.0.as_secs() % 60;
        let minutes = (self.0.as_secs() / 60) % 60;
        let hours = self.0.as_secs() / 3600;

        let s = format!("{hours:02}:{minutes:02}:{seconds:02}");
        f.pad(&s)
    }
}

pub struct History {
    entries: Vec<Entry>,
    names: Vec<String>,
}

impl History {
    pub fn new(entries: Vec<Entry>, config: Config) -> Self {
        History {
            entries,
            names: config
                .sides
                .iter()
                .enumerate()
                .map(|(i, side)| {
                    if let Some(name) = &side.name {
                        name.clone()
                    } else {
                        format!("Side {i}")
                    }
                })
                .collect(),
        }
    }

    pub fn all<'a>(&'a self) -> HistoryFiltered<'a> {
        HistoryFiltered {
            entries: self.entries.iter().collect(),
            names: &self.names,
        }
    }

    pub fn since<'a>(&'a self, date: DateTime<Utc>) -> HistoryFiltered<'a> {
        HistoryFiltered {
            entries: self
                .entries
                .iter()
                .filter(|entry| entry.time > date)
                .collect(),
            names: &self.names,
        }
    }
}

pub struct HistoryFiltered<'a> {
    entries: Vec<&'a Entry>,
    names: &'a [String],
}

impl<'a> HistoryFiltered<'a> {
    pub fn table(&'a self) -> HistoryTable<'a> {
        HistoryTable {
            entries: &self.entries,
            names: self.names,
        }
    }
}

impl<'a> fmt::Display for HistoryFiltered<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let timezone = Local::now().timezone();

        for entry in &self.entries {
            writeln!(
                f,
                "{}",
                EntryView {
                    entry,
                    name: &self.names[usize::from(entry.facet.index()) - 1],
                    timezone: &timezone,
                    align_name: 10,
                    with_id: true,
                },
            )?;
        }

        Ok(())
    }
}

struct EntryView<'a, 'b, T: TimeZone> {
    entry: &'a Entry,
    name: &'a str,
    timezone: &'b T,

    align_name: usize,
    with_id: bool,
}

impl<'a, 'b, T> fmt::Display for EntryView<'a, 'b, T>
where
    T: TimeZone,
    <T as TimeZone>::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = format!(
            "{:>align_name$}{}: {} on {} for {} seconds",
            self.name,
            if self.with_id {
                format!(" ({})", self.entry.id)
            } else {
                "".into()
            },
            if self.entry.pause {
                " paused"
            } else {
                "started"
            },
            self.entry.time.with_timezone(self.timezone),
            self.entry.duration.as_secs(),
            align_name = self.align_name
        );

        f.pad(&line)
    }
}

pub struct HistoryTable<'a> {
    entries: &'a [&'a Entry],
    names: &'a [String],
}

impl<'a> fmt::Display for HistoryTable<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let timezone = Local::now().timezone();
        const WIDTH_NAME: usize = 15;
        const WIDTH_STARTED: usize = 30;
        const WIDTH_DURATION: usize = 10;

        writeln!(
            f,
            "{}",
            TableHeader {
                columns: vec![
                    ("Side", WIDTH_NAME),
                    ("Started", WIDTH_STARTED),
                    ("Duration", WIDTH_DURATION)
                ],
                position: Position::Top,
            },
        )?;

        for entry in self.entries {
            if entry.pause {
                continue;
            }
            writeln!(
                f,
                "│ {} │",
                EntryTableView {
                    entry,
                    name: &self.names[usize::from(entry.facet.index() - 1)],
                    timezone: &timezone,
                    separator: "│",
                    width_name: WIDTH_NAME,
                    width_started: WIDTH_STARTED,
                    width_duration: WIDTH_DURATION,
                },
            )?;
        }

        writeln!(
            f,
            "{}",
            TableHeader {
                columns: vec![("", WIDTH_NAME), ("", WIDTH_STARTED), ("", WIDTH_DURATION)],
                position: Position::Bottom,
            },
        )?;

        Ok(())
    }
}

struct EntryTableView<'a, 'b, T: TimeZone> {
    entry: &'a Entry,
    name: &'a str,
    timezone: &'b T,

    separator: &'b str,
    width_name: usize,
    width_started: usize,
    width_duration: usize,
}

impl<'a, 'b, T> fmt::Display for EntryTableView<'a, 'b, T>
where
    T: TimeZone,
    <T as TimeZone>::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = format!(
            "{:<width_name$}{}{:<width_started$}{}{:>width_duration$}",
            self.name,
            self.separator,
            self.entry.time.with_timezone(self.timezone).to_string(),
            self.separator,
            DurationView(&self.entry.duration),
            width_name = self.width_name,
            width_started = self.width_started,
            width_duration = self.width_duration,
        );

        f.pad(&line)
    }
}
