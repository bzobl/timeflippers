use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use std::{collections::HashMap, fmt, time::Duration};

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
                .filter(|entry| !entry.pause && entry.time > date)
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
    fn group_by_day(&self) -> Vec<(NaiveDate, Vec<&Entry>)> {
        let timezone = Local::now().timezone();

        let mut groups = HashMap::<NaiveDate, Vec<&Entry>>::new();
        for entry in &self.entries {
            groups
                .entry(entry.time.with_timezone(&timezone).date_naive())
                .or_default()
                .push(entry);
        }

        let mut sorted = groups.into_iter().collect::<Vec<_>>();
        sorted.sort_by(|(date_a, _), (date_b, _)| date_a.cmp(date_b));
        sorted
    }

    pub fn table(&'a self) -> HistoryTable<'a> {
        HistoryTable {
            groups: vec![(None, self.entries.clone())],
            names: self.names,
        }
    }

    pub fn table_by_day(&'a self) -> HistoryTable<'a> {
        let groups = self
            .group_by_day()
            .into_iter()
            .map(|(date, entries)| (Some(format!(" {} ", date)), entries))
            .collect();

        HistoryTable {
            groups,
            names: self.names,
        }
    }

    pub fn summarized(&self) -> Summarized {
        let groups = self
            .group_by_day()
            .into_iter()
            .map(|(date, entries)| {
                let mut durations = HashMap::<String, Duration>::new();
                for entry in entries.into_iter() {
                    let sum = durations
                        .entry(self.names[entry.facet.index_zero()].clone())
                        .or_default();
                    *sum = sum.saturating_add(entry.duration);
                }

                (date, durations)
            })
            .collect();
        Summarized { groups }
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
                    name: &self.names[entry.facet.index_zero()],
                    timezone: &timezone,
                    align_name: 12,
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
    groups: Vec<(Option<String>, Vec<&'a Entry>)>,
    names: &'a [String],
}

impl<'a> fmt::Display for HistoryTable<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const WIDTH_NAME: usize = 15;
        const WIDTH_STARTED: usize = 30;
        const WIDTH_DURATION: usize = 10;

        writeln!(
            f,
            "{}",
            TableHeader {
                columns: vec![
                    (" Side ", WIDTH_NAME),
                    (" Started ", WIDTH_STARTED),
                    (" Duration ", WIDTH_DURATION)
                ],
                position: Position::Top,
            },
        )?;

        for (name, entries) in &self.groups {
            write!(
                f,
                "{}",
                GroupTable {
                    group: name.as_deref(),
                    entries: &entries[..],
                    names: &self.names,
                }
            )?;
        }

        write!(
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

struct GroupTable<'a> {
    group: Option<&'a str>,
    entries: &'a [&'a Entry],
    names: &'a [String],
}

impl<'a> fmt::Display for GroupTable<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let timezone = Local::now().timezone();
        const WIDTH_NAME: usize = 15;
        const WIDTH_STARTED: usize = 30;
        const WIDTH_DURATION: usize = 10;

        if let Some(group_name) = self.group {
            writeln!(
                f,
                "{}",
                TableHeader {
                    columns: vec![
                        ("", WIDTH_NAME),
                        (group_name, WIDTH_STARTED),
                        ("", WIDTH_DURATION),
                    ],
                    position: Position::Center,
                }
            )?;
        }

        for entry in self.entries {
            writeln!(
                f,
                "│ {} │",
                EntryTableView {
                    entry,
                    name: &self.names[entry.facet.index_zero()],
                    timezone: &timezone,
                    separator: "│",
                    width_name: WIDTH_NAME,
                    width_started: WIDTH_STARTED,
                    width_duration: WIDTH_DURATION,
                },
            )?;
        }

        Ok(())
    }
}

struct EntryTableView<'a, T: TimeZone> {
    entry: &'a Entry,
    name: &'a str,
    timezone: &'a T,

    separator: &'a str,
    width_name: usize,
    width_started: usize,
    width_duration: usize,
}

impl<'a, T> fmt::Display for EntryTableView<'a, T>
where
    T: TimeZone,
    <T as TimeZone>::Offset: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = format!(
            "{:<width_name$}{}{:^width_started$}{}{:>width_duration$}",
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

pub struct Summarized {
    groups: Vec<(NaiveDate, HashMap<String, Duration>)>,
}

impl fmt::Display for Summarized {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const WIDTH_NAME: usize = 20;
        const WIDTH_DURATION: usize = 10;

        writeln!(
            f,
            "{}",
            TableHeader {
                columns: vec![(" Side ", WIDTH_NAME), (" Duration ", WIDTH_DURATION)],
                position: Position::Top,
            },
        )?;

        for (time, durations) in self.groups.iter() {
            writeln!(
                f,
                "{}",
                TableHeader {
                    columns: vec![(&time.to_string(), WIDTH_NAME), ("", WIDTH_DURATION)],
                    position: Position::Center,
                },
            )?;

            for (facet, duration) in durations.iter() {
                writeln!(
                    f,
                    "│ {:<width_name$}│{:>width_duration$} │",
                    facet,
                    DurationView(&duration),
                    width_name = WIDTH_NAME,
                    width_duration = WIDTH_DURATION,
                )?;
            }
        }

        write!(
            f,
            "{}",
            TableHeader {
                columns: vec![("", WIDTH_NAME), ("", WIDTH_DURATION)],
                position: Position::Bottom,
            },
        )
    }
}
