use chrono::offset::Local;
use clap::{Parser, Subcommand, ValueEnum};
use futures::stream::StreamExt;
use timeflippers::{
    timeflip::{Error, TimeFlip},
    BluetoothSession, Config, Facet,
};
use tokio::{select, signal};

#[derive(Parser)]
struct Options {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    Battery,
    Facet,
    Status,
    SyncState,
    Sync,
    Time,
    WriteConfig,
}

impl Command {
    async fn run(&self, timeflip: &mut TimeFlip) -> Result<(), Error> {
        use Command::*;
        match self {
            Battery => {
                println!("Battery level: {}%", timeflip.battery_level().await?);
            }
            Facet => {
                println!("Currently up: {:?}", timeflip.facet().await?);
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
            WriteConfig => {
                // TODO read config from file
                let config = Config::default();
                timeflip.write_config(config).await?;
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
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

    return Ok(());

    log::info!("Last event: {}", timeflip.last_event().await?);
    log::info!("Reading history");
    let history = timeflip.read_history_since(270).await?;
    for entry in history {
        log::info!("{}", entry);
    }
    log::info!(
        "Reading last event: {}",
        timeflip.read_last_history_entry().await?
    );

    log::info!(
        "Settings of Facet(1): {:?}",
        timeflip.get_task(Facet::new(1).unwrap()).await?,
    );

    timeflip.subscribe_battery_level().await?;
    timeflip.subscribe_events().await?;
    timeflip.subscribe_facet().await?;
    timeflip.subscribe_double_tap().await?;
    let mut stream = timeflip.event_stream().await?;

    log::info!("Waiting for events");

    loop {
        select! {
            event = stream.next() => {
                log::info!("New event: {event:?}");
            }
            _ = signal::ctrl_c() => {
                log::info!("shutting down");
                break;
            }
            res = &mut bg_task => {
                if let Err(e) =res {
                    log::error!("bluetooth session background task exited with error: {e}");
                }
                break;
            }
        }
    }

    //timeflip.disconnect().await?;

    Ok(())
}
