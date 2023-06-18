use chrono::{offset::Local, TimeZone};
use timeflippers::{
    timeflip::{BlinkInterval, Error, Facet, TimeFlip},
    BluetoothSession,
};
use tokio::{select, signal};

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    let tz = Local::now().timezone();

    let (bg_task, session) = BluetoothSession::new().await?;

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
    let history = timeflip.read_history_since(200).await?;
    for entry in history {
        log::info!("{}", entry);
    }
    log::info!(
        "Reading last event: {}",
        timeflip.read_history_entry(0xFFFFFFFF).await?
    );

    log::info!(
        "Settings of Facet(1): {:?}",
        timeflip.get_task(Facet::new(1).unwrap()).await?,
    );

    timeflip.color(Facet::new(1).unwrap(), 0, 0, 0xffff).await?;
    timeflip
        .blink_interval(BlinkInterval::new(30).unwrap())
        .await?;
    timeflip.pause().await?;
    log::info!("pause");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    timeflip.unpause().await?;
    log::info!("unpaused");

    select! {
        _ = signal::ctrl_c() => {
            log::info!("shutting down");
        }
        res = bg_task => {
            if let Err(e) =res {
                log::error!("bluetooth session background task exited with error: {e}");
            }
        }
    }

    //timeflip.disconnect().await?;

    Ok(())
}
