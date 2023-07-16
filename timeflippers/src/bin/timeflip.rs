use chrono::{offset::Local, DateTime, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use futures::StreamExt;
use std::{
    io,
    path::{Path, PathBuf},
};
use timeflippers::{
    timeflip::{Entry, Event, TimeFlip},
    view, BluetoothSession, Config,
};
use tokio::{fs, select, signal};

async fn read_config(path: impl AsRef<Path>) -> anyhow::Result<Config> {
    let toml = fs::read_to_string(path).await?;
    let config: Config = toml::from_str(&toml)?;
    Ok(config)
}

/// Communicate with a TimeFlip2 cube.
///
/// Note: Use `bluetoothctl` to pair (and potentially connect) the TimeFlip2.
/// Currently, the TimeFlip2's password is expected to be the default value.
#[derive(Parser)]
#[clap(about)]
struct Options {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum HistoryStyle {
    Lines,
    Tabular,
    Summarized,
}

#[derive(Subcommand)]
enum Command {
    /// Print the current battery level.
    Battery,
    /// Print logged TimeFlip events.
    History {
        #[arg(help = "path to the timeflip.toml file")]
        config: PathBuf,
        #[arg(long, help = "read events from and write new events to file")]
        update: Option<PathBuf>,
        #[arg(
            long,
            help = "start reading with entry ID, latest event in `--update` takes precedence",
            default_value = "0"
        )]
        start_with: u32,
        #[arg(long, help = "start displaying with entries after DATE (YYYY-MM-DD)")]
        since: Option<NaiveDate>,
        #[arg(long, help = "choose output style", default_value = "tabular")]
        style: HistoryStyle,
    },
    /// Print the facet currently facing up.
    Facet,
    /// Put the TimeFlip2 in lock mode.
    Lock,
    /// Release the TimeFlip2 from lock mode.
    Unlock,
    /// Subscribe to properties and get notified if they change.
    Notify {
        #[arg(long, help = "listen for battery events")]
        battery: bool,
        #[arg(long, help = "listen for facet events")]
        facet: bool,
        #[arg(long, help = "listen for double-tap events")]
        double_tap: bool,
        #[arg(long, help = "listen for log events")]
        log_event: bool,
    },
    /// Put the TimeFlip2 into pause mode.
    Pause,
    /// Release the TimeFlip2 from pause mode.
    Unpause,
    /// Print the TimeFlip2's system status.
    Status,
    /// Get the TimeFlip2's synchronization state.
    SyncState,
    /// Synchronize TimeFlip2. Do nothing if the cube reports it is synchronized.
    Sync {
        #[arg(help = "path to the timeflip.toml file")]
        config: PathBuf,
    },
    /// Get the TimeFlip2's current time.
    Time,
    /// Write config from the toml file to the TimeFlip2's memory.
    WriteConfig {
        #[arg(help = "path to the timeflip.toml file")]
        config: PathBuf,
    },
}

impl Command {
    async fn run(&self, timeflip: &mut TimeFlip) -> anyhow::Result<()> {
        use Command::*;
        match self {
            Battery => {
                println!("Battery level: {}%", timeflip.battery_level().await?);
            }
            History {
                config,
                update: update_file,
                start_with,
                style,
                since,
            } => {
                let config = read_config(config).await?;

                let (start_with, mut entries) = if let Some(file) = update_file {
                    match fs::read_to_string(file).await {
                        Ok(s) => {
                            let mut entries: Vec<Entry> = serde_json::from_str(&s)?;
                            entries.sort_by(|a, b| a.id.cmp(&b.id));
                            (entries.last().map(|e| e.id).unwrap_or(0), entries)
                        }
                        Err(e) if e.kind() == io::ErrorKind::NotFound => (0, vec![]),
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    (*start_with, vec![])
                };

                let mut update = timeflip.read_history_since(start_with).await?;

                let new_ids = update.iter().map(|e| e.id).collect::<Vec<_>>();
                entries.retain(|entry| !new_ids.contains(&entry.id));
                entries.append(&mut update);

                if let Some(file) = update_file {
                    match serde_json::to_vec(&entries) {
                        Ok(json) => {
                            if let Err(e) = fs::write(file, json).await {
                                eprintln!("cannot update entries file {}: {e}", file.display());
                            }
                        }
                        Err(e) => eprintln!("cannot update entries file {}: {e}", file.display()),
                    }
                }

                let history = view::History::new(entries, config);
                let filtered = if let Some(since) = since {
                    let date = DateTime::<Local>::from_local(
                        since.and_hms_opt(0, 0, 0).expect("is a valid time"),
                        *Local::now().offset(),
                    );

                    history.since(date.into())
                } else {
                    history.all()
                };
                use HistoryStyle::*;
                match style {
                    Lines => println!("{}", filtered),
                    Tabular => println!("{}", filtered.table_by_day()),
                    Summarized => println!("{}", filtered.summarized()),
                }
            }
            Facet => {
                println!("Currently up: {:?}", timeflip.facet().await?);
            }
            Lock => timeflip.lock().await?,
            Unlock => timeflip.unlock().await?,
            Notify {
                battery,
                facet,
                double_tap,
                log_event,
            } => {
                if *battery {
                    timeflip.subscribe_battery_level().await?;
                }
                if *facet {
                    timeflip.subscribe_facet().await?;
                }
                if *double_tap {
                    timeflip.subscribe_double_tap().await?;
                }
                if *log_event {
                    timeflip.subscribe_events().await?;
                }

                let mut stream = timeflip.event_stream().await?;
                loop {
                    match stream.next().await {
                        Some(Event::BatteryLevel(percent)) => println!("Battery Level {percent}"),
                        Some(Event::Event(event)) => println!("{event}"),
                        Some(Event::Facet(facet)) => println!("Currently Up: {facet}"),
                        Some(Event::DoubleTap { facet, pause }) => println!(
                            "Facet {facet} has {}",
                            if pause { "paused" } else { "started" }
                        ),
                        Some(Event::Disconnected) => {
                            println!("TimeFlip has disconnected");
                            break;
                        }
                        None => break,
                    }
                }
            }
            Pause => timeflip.pause().await?,
            Unpause => timeflip.unpause().await?,
            Status => {
                println!("System status: {:?}", timeflip.system_status().await?);
            }
            SyncState => {
                println!("Sync state: {:?}", timeflip.sync_state().await?);
            }
            Sync { config } => {
                let config = read_config(config).await?;
                timeflip.sync(&config).await?;
            }
            Time => {
                let tz = Local::now().timezone();
                let time = timeflip.time().await?;
                println!("Time set on TimeFlip: {}", time.with_timezone(&tz));
            }
            WriteConfig { config } => {
                let config = read_config(config).await?;
                timeflip.write_config(config).await?;
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let opt = Options::parse();

    let (mut bg_task, session) = BluetoothSession::new().await?;

    let mut timeflip = TimeFlip::connect(&session).await?;
    log::info!("connected");

    select! {
        _ = signal::ctrl_c() => {
            log::info!("shutting down");
        }
        res = &mut bg_task => {
            if let Err(e) =res {
                log::error!("bluetooth session background task exited with error: {e}");
            }
        }
        res = opt.cmd.run(&mut timeflip) => {
            res?;
        }
    }

    Ok(())
}
