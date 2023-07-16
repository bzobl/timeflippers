use chrono::{offset::Local, DateTime, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use timeflippers::{
    timeflip::{Event, TimeFlip},
    view::History,
    BluetoothSession, Config,
};
use tokio::{fs, select, signal};

async fn read_config(path: impl AsRef<Path>) -> anyhow::Result<Config> {
    let toml = fs::read_to_string(path).await?;
    let config: Config = toml::from_str(&toml)?;
    Ok(config)
}

#[derive(Parser)]
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
    Battery,
    Events {
        #[arg(help = "path to the timeflip.toml file")]
        config: PathBuf,
        #[arg(long, help = "start reading with entry ID", default_value = "0")]
        start_with: u32,
        #[arg(long, help = "start displaying with entries after DATE (YYYY-MM-DD)")]
        since: Option<NaiveDate>,
        #[arg(long, help = "choose output style", default_value = "tabular")]
        style: HistoryStyle,
    },
    Facet,
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
    Status,
    SyncState,
    Sync,
    Time,
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
            Events {
                config,
                start_with,
                style,
                since,
            } => {
                let config = read_config(config).await?;

                let entries = timeflip.read_history_since(*start_with).await?;
                let history = History::new(entries, config);
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
                    Tabular => println!("{}", filtered.table()),
                }
            }
            Facet => {
                println!("Currently up: {:?}", timeflip.facet().await?);
            }
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
            Status => {
                println!("System status: {:?}", timeflip.system_status().await?);
            }
            SyncState => {
                println!("Sync state: {:?}", timeflip.sync_state().await?);
            }
            Sync => {
                // TODO read config from file
                let config = Config::default();
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
