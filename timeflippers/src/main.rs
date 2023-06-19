use chrono::{offset::Local, TimeZone};
use futures::stream::StreamExt;
use timeflippers::{
    timeflip::{Error, Facet, TimeFlip},
    BluetoothSession,
};
use tokio::{select, signal};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let tz = Local::now().timezone();

    let (mut bg_task, session) = BluetoothSession::new().await?;

    let timeflip = TimeFlip::connect(&session).await?;
    log::info!("connected");
    log::info!("Battery level: {}%", timeflip.battery_level().await?);
    log::info!("Sync state: {:?}", timeflip.sync_state().await?);
    log::info!("Last event: {}", timeflip.last_event().await?);
    log::info!("Facet: {:?}", timeflip.facet().await?);
    let time = timeflip.time().await?;
    log::info!("Time set on TimeFlip: {}", tz.from_utc_datetime(&time));
    log::info!("System status: {:?}", timeflip.system_status().await?);
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
