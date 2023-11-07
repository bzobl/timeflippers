//! Communicating with TimeFlip2
#![deny(missing_docs)]

use bluez_async::{
    BluetoothError, BluetoothEvent, BluetoothSession, CharacteristicEvent, CharacteristicInfo,
    DeviceInfo,
};
use bytes::BufMut;
use chrono::{DateTime, Utc};
use futures::stream::{BoxStream, StreamExt};
use std::{convert::Infallible, string::FromUtf8Error};
use thiserror::Error;

use crate::{
    config::Config,
    types::{BlinkInterval, Color, Facet, FacetError, FacetTask, Minutes, Percent, PercentError},
};

mod gatt;
pub use gatt::{Entry, Event, FacetSettings, SyncState, SyncType, SystemStatus};

/// Error for communication with TimeFlip2.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum Error {
    #[error("characteristic read returned insufficient data, read {0} of {1}")]
    ReadTooShort(usize, usize),
    #[error("invalid command in result: {0}")]
    InvalidCommand(u8),
    #[error("command execution failed")]
    CommandExecutionFailed,
    #[error("{0}")]
    GetTime(#[from] gatt::GetTimeError),
    #[error("{0}")]
    Utf8Error(#[from] FromUtf8Error),
    #[error("invalid battery level: {0}")]
    InvalidBatteryLevel(PercentError),
    #[error("invalid facet: {0}")]
    InvalidFacet(#[from] FacetError),
    #[error("invalid facet settings: {0}")]
    InvalidFacetSettings(#[from] gatt::FacetSettingsError),
    #[error("characteristic read returned invalid data: {0}")]
    InvalidCharacteristicData(String),
    #[error("invalid history entry: {0}")]
    InvalidHistoryEntry(#[from] gatt::EntryError),
    #[error("invalid sync state: {0}")]
    InvalidSyncState(#[from] gatt::SyncStateError),
    #[error("invalid system status: {0}")]
    InvalidSystemStatus(#[from] gatt::SystemStatusError),
    #[error("{0}")]
    Bluetooth(#[from] BluetoothError),
    #[error("no TimeFlip2 bluetooth device found")]
    NoDevice,
    #[error("TimeFlip2 reports Accelerometer error")]
    AccelerometerError,
    #[error("TimeFlip2 reports Flash error")]
    FlashError,
    #[error("Could not synchronize {0:?}")]
    SyncError(SyncType),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!("infallible")
    }
}

/// Handles to TimeFlip2's characteristics.
///
/// We need the CharacteristicInfo, which is bound to the bluez device, for accessing the dice's
/// attributes, hence we have to query it once during initialization.
#[derive(Debug, Clone)]
struct CharacteristicHandles {
    battery_level: CharacteristicInfo,
    event: CharacteristicInfo,
    facet: CharacteristicInfo,
    command_result: CharacteristicInfo,
    command: CharacteristicInfo,
    double_tap: CharacteristicInfo,
    system_state: CharacteristicInfo,
    password: CharacteristicInfo,
    history: CharacteristicInfo,
}

/// Representation of a TimeFlip2 dice connected via Bluetooth.
#[derive(Debug)]
pub struct TimeFlip {
    /// Handle to the dbus session communicating with bluez.
    session: BluetoothSession,
    /// Handle for the TimeFlip2 Bluetooth device
    device: DeviceInfo,
    /// Handle to each of the device's characteristics.
    characteristics: CharacteristicHandles,
    /// Password to write to the TimeFlip2's password characteristic when connecting.
    password: [u8; 6],
}

impl TimeFlip {
    /// Discover devices announcing the TimeFlip service and connect to it.
    ///
    /// Currently, the first TimeFlip2 encountered is selected.
    ///
    /// Pairing should be done with `bluetoothctl` first.
    ///
    /// FIXME: Connecting does not work reliably yet. If in doubt, pair and connect via
    /// `bluetoothctl` first.
    pub async fn connect(
        session: &BluetoothSession,
        password: Option<[u8; 6]>,
    ) -> Result<Self, Error> {
        let time_flip_service_id = gatt::Service::TimeFlip.uuid();

        let device = if let Some(device) = session.get_devices().await?.into_iter().find(|dev| {
            log::debug!(
                "found device {} ({})",
                dev.name.as_deref().unwrap_or("<unknown>"),
                dev.mac_address
            );
            dev.services
                .iter()
                .any(|service| *service == time_flip_service_id)
        }) {
            device
        } else {
            // If the TimeFlip2 is paired, it should be present in the adapter's device list
            // regardless of whether or not it is in range.
            //
            // It seems that bluez_async does not support pairing at the moment, hence we rely
            // on bluetoothctl for that.
            log::warn!(
                "no devices are found, this probably means the TimeFlip2 is not paired,
                 please pair via `bluetoothctl`"
            );
            return Err(Error::NoDevice);
        };

        if !device.paired {
            log::warn!("device is not paired");
        }

        log::info!(
            "selected device {} ({}) for TimeFlip2",
            device.name.as_deref().unwrap_or("<unknown>"),
            device.mac_address
        );

        if device.connected {
            log::debug!("already connected");
        } else {
            log::info!("currently not connected, will connect");
            session.connect(&device.id).await?;
        }

        use gatt::Characteristic::*;
        let id = device.id.clone();
        let timeflip = TimeFlip {
            session: session.clone(),
            device,
            characteristics: CharacteristicHandles {
                battery_level: BatteryLevel.get_info(session, &id).await?,
                event: Event.get_info(session, &id).await?,
                facet: Facet.get_info(session, &id).await?,
                command_result: CommandResult.get_info(session, &id).await?,
                command: Command.get_info(session, &id).await?,
                double_tap: DoubleTap.get_info(session, &id).await?,
                system_state: SystemState.get_info(session, &id).await?,
                password: Password.get_info(session, &id).await?,
                history: History.get_info(session, &id).await?,
            },
            password: password.unwrap_or([0x30; 6]),
        };

        timeflip.write_password().await?;

        Ok(timeflip)
    }

    /// Disconnect the bluetooth device.
    pub async fn disconnect(&self) -> Result<(), Error> {
        Ok(self.session.disconnect(&self.device.id).await?)
    }

    /// Write the password to access TimeFlip2's properties properly.
    async fn write_password(&self) -> Result<(), Error> {
        log::debug!("writing password");
        self.session
            .write_characteristic_value(&self.characteristics.password.id, self.password)
            .await?;
        Ok(())
    }

    /// Get the TimeFlip2's battery level in percent.
    pub async fn battery_level(&self) -> Result<Percent, Error> {
        let data = self
            .session
            .read_characteristic_value(&self.characteristics.battery_level.id)
            .await?;

        match data.first() {
            Some(v) => Percent::new((*v).into()).map_err(Error::InvalidBatteryLevel),
            None => Err(Error::ReadTooShort(data.len(), 1)),
        }
    }

    /// Subscribe for [Event::BatteryLevel] events.
    pub async fn subscribe_battery_level(&self) -> Result<(), Error> {
        self.session
            .start_notify(&self.characteristics.battery_level.id)
            .await
            .map_err(Into::into)
    }

    /// Read the (informational) last event of the TimeFlip2.
    pub async fn last_event(&self) -> Result<String, Error> {
        let data = self
            .session
            .read_characteristic_value(&self.characteristics.event.id)
            .await?;

        String::from_utf8(data).map_err(Into::into)
    }

    /// Subscribe for [Event::Event] events.
    pub async fn subscribe_events(&self) -> Result<(), Error> {
        self.session
            .start_notify(&self.characteristics.event.id)
            .await
            .map_err(Into::into)
    }

    /// The facet currently facing up.
    pub async fn facet(&self) -> Result<Facet, Error> {
        let data = self
            .session
            .read_characteristic_value(&self.characteristics.facet.id)
            .await?;

        match data.first() {
            Some(facet) => Ok(Facet::new(usize::from(*facet))?),
            None => Err(Error::ReadTooShort(data.len(), 1)),
        }
    }

    /// Subscribe for [Event::Facet] events.
    pub async fn subscribe_facet(&self) -> Result<(), Error> {
        self.session
            .start_notify(&self.characteristics.facet.id)
            .await
            .map_err(Into::into)
    }

    /// Subscribe for [Event::DoubleTap] events.
    pub async fn subscribe_double_tap(&self) -> Result<(), Error> {
        self.session
            .start_notify(&self.characteristics.double_tap.id)
            .await
            .map_err(Into::into)
    }

    /// Write a command to TimeFlip2, check its execution and read its output from the
    /// CommandResult characteristic.
    async fn command<T>(
        &self,
        command: gatt::Command,
    ) -> Result<<T as gatt::CommandResult>::Output, Error>
    where
        T: gatt::CommandResult,
        Error: From<T::Error>,
    {
        self.session
            .write_characteristic_value(&self.characteristics.command.id, command.to_vec())
            .await?;
        let cmd_execution = self
            .session
            .read_characteristic_value(&self.characteristics.command.id)
            .await?;
        if cmd_execution.len() < 2 || cmd_execution[0] != command.id() || cmd_execution[1] != 2 {
            return Err(Error::CommandExecutionFailed);
        }

        let data = self
            .session
            .read_characteristic_value(&self.characteristics.command_result.id)
            .await?;
        T::from_data(data.as_slice()).map_err(Into::into)
    }

    /// Get the current time (in UTC) saved on TimeFlip2.
    pub async fn time(&self) -> Result<DateTime<Utc>, Error> {
        self.command::<DateTime<Utc>>(gatt::Command::GetTime).await
    }

    /// Set the time (in UTC) saved on TimeFlip2.
    pub async fn set_time(&self, time: DateTime<Utc>) -> Result<(), Error> {
        self.command::<()>(gatt::Command::Time(time)).await
    }

    /// Get the system status of the TimeFlip2.
    pub async fn system_status(&self) -> Result<SystemStatus, Error> {
        self.command::<SystemStatus>(gatt::Command::ReadStatus)
            .await
    }

    /// Set the brightness of the TimeFlip2's LED.
    pub async fn brightness(&self, value: Percent) -> Result<(), Error> {
        log::info!("writing brightness {value} to TimeFlip2");
        self.command::<()>(gatt::Command::Brightness(value)).await
    }

    /// Set the blink interval of the TimeFlip2's LED.
    pub async fn blink_interval(&self, value: BlinkInterval) -> Result<(), Error> {
        log::info!("writing blink interval {value} to TimeFlip2");
        self.command::<()>(gatt::Command::BlinkInterval(value))
            .await
    }

    /// Set the color of a facet's LED.
    pub async fn color(&self, facet: Facet, color: Color) -> Result<(), Error> {
        log::info!("writing color of facet {facet}: {color}");
        self.command::<()>(gatt::Command::SetColor { facet, color })
            .await
    }

    /// Set a facet's task.
    pub async fn task(&self, facet: Facet, task: FacetTask) -> Result<(), Error> {
        log::info!("writing task of facet {facet}: {task}");
        self.command::<()>(gatt::Command::SetTaskParameter(facet, task))
            .await
    }

    /// Get a facet's task.
    pub async fn get_task(&self, facet: Facet) -> Result<FacetSettings, Error> {
        self.command::<FacetSettings>(gatt::Command::GetTaskParameter(facet))
            .await
    }

    /// Put the TimeFlip2 into lock mode.
    pub async fn lock(&self) -> Result<(), Error> {
        log::info!("locking TimeFlip2");
        self.command::<()>(gatt::Command::LockMode(true)).await
    }

    /// Release the TimeFlip2 from lock mode.
    pub async fn unlock(&self) -> Result<(), Error> {
        log::info!("unlocking TimeFlip2");
        self.command::<()>(gatt::Command::LockMode(false)).await
    }

    /// Put the TimeFlip2 into pause mode.
    pub async fn pause(&self) -> Result<(), Error> {
        log::info!("pausing TimeFlip2");
        self.command::<()>(gatt::Command::PauseMode(true)).await
    }

    /// Release the TimeFlip2 from pause mode.
    pub async fn unpause(&self) -> Result<(), Error> {
        log::info!("unpausing TimeFlip2");
        self.command::<()>(gatt::Command::PauseMode(false)).await
    }

    /// Set the TimeFlip2's auto pause time.
    pub async fn auto_pause(&self, time: Minutes) -> Result<(), Error> {
        log::info!("writing auto pause after {time} to TimeFlip2");
        self.command::<()>(gatt::Command::AutoPauseTime(time)).await
    }

    /// Get the TimeFlip2's sync state.
    pub async fn sync_state(&self) -> Result<SyncState, Error> {
        let data = self
            .session
            .read_characteristic_value(&self.characteristics.system_state.id)
            .await?;
        SyncState::from_data(&data).map_err(Into::into)
    }

    /// Synchronize the TimeFlip2 to the given config.
    ///
    /// Please note that this will not apply the configuration unconditionally, but only if
    /// TimeFlip requires synchronization. When attempting to apply configuration use
    /// [TimeFlip::set_config()] instead.
    pub async fn sync(&self, config: &Config) -> Result<(), Error> {
        let mut last_sync = None;
        loop {
            let sync_state = self.sync_state().await?;
            if sync_state.accelerometer_error {
                return Err(Error::AccelerometerError);
            }
            if sync_state.flash_error {
                return Err(Error::FlashError);
            }

            if let Some(last_sync) = last_sync {
                if last_sync == sync_state.sync {
                    return Err(Error::SyncError(last_sync));
                }
            }

            use SyncType::*;
            match sync_state.sync {
                FactoryReset | Time => {
                    self.set_time(Utc::now()).await?;
                }
                FacetColor => {
                    for (i, side) in config.sides.iter().enumerate() {
                        let facet = Facet::new(i + 1)?;
                        self.color(facet, side.color.clone()).await?;
                    }
                }
                LedBrightness => {
                    self.brightness(config.brightness.clone()).await?;
                }
                BlinkInterval => {
                    self.blink_interval(config.blink_interval.clone()).await?;
                }
                TaskParameters => {
                    for (i, side) in config.sides.iter().enumerate() {
                        let facet = Facet::new(i + 1)?;
                        self.task(facet, side.task.clone()).await?;
                    }
                }
                AutoPause => {
                    self.auto_pause(config.auto_pause.clone()).await?;
                }
                Synchronized => return Ok(()),
            }
            last_sync = Some(sync_state.sync);
        }
    }

    /// Apply the given configuration to TimeFlip2's memory.
    pub async fn write_config(&self, config: Config) -> Result<(), Error> {
        // TODO: write password
        self.brightness(config.brightness).await?;
        self.blink_interval(config.blink_interval).await?;
        self.auto_pause(config.auto_pause).await?;
        for (i, side) in config.sides.into_iter().enumerate() {
            let facet = Facet::new(i + 1)?;
            self.color(facet.clone(), side.color).await?;
            self.task(facet.clone(), side.task).await?;
        }

        Ok(())
    }

    /// Read a single history event identified by its ID.
    ///
    /// When `0xFFFFFFFF` is passed as `id`, the last event is returned.
    pub async fn read_history_entry(&self, id: u32) -> Result<Entry, Error> {
        let mut read_command = Vec::with_capacity(5);
        read_command.put_u8(0x01);
        read_command.put_u32(id);
        self.session
            .write_characteristic_value(&self.characteristics.history.id, read_command)
            .await?;
        let data = self
            .session
            .read_characteristic_value(&self.characteristics.history.id)
            .await?;

        Ok(Entry::from_data(&data)?)
    }

    /// Read the last histroy entry.
    pub async fn read_last_history_entry(&self) -> Result<Entry, Error> {
        self.read_history_entry(0xFFFF_FFFF).await
    }

    /// Read history entries.
    ///
    /// Please note that TimeFlip2 will only consider events with a duration of more than 5
    /// seconds.
    pub async fn read_history_since(&self, id: u32) -> Result<Vec<Entry>, Error> {
        self.session
            .start_notify(&self.characteristics.history.id)
            .await?;
        let mut stream = self
            .session
            .characteristic_event_stream(&self.characteristics.history.id)
            .await?;

        let mut read_command = Vec::with_capacity(5);
        read_command.put_u8(0x02);
        read_command.put_u32(id);
        self.session
            .write_characteristic_value(&self.characteristics.history.id, read_command)
            .await?;

        let mut entries = vec![];
        while let Some(event) = stream.next().await {
            match event {
                BluetoothEvent::Characteristic {
                    id,
                    event: CharacteristicEvent::Value { value },
                } => {
                    if id != self.characteristics.history.id {
                        return Err(Error::InvalidCharacteristicData(format!(
                            "wrong ID in bluetooth event {:?}",
                            id
                        )));
                    }
                    match Entry::from_data(&value) {
                        Ok(entry) => {
                            log::debug!("new entry: {entry}");
                            entries.push(entry);
                        }
                        Err(gatt::EntryError::EndOfHistory) => break,
                        Err(e) => log::error!("skipping unparsable history event: {e}"),
                    }
                }
                _ => {
                    return Err(Error::InvalidCharacteristicData(format!(
                        "invalid bluetooth event {:?}",
                        event
                    )))
                }
            }
        }

        self.session
            .stop_notify(&self.characteristics.history.id)
            .await?;

        Ok(entries)
    }

    /// Get a stream of events from TimeFlip2.
    pub async fn event_stream(&self) -> Result<BoxStream<'_, Event>, Error> {
        let handles = gatt::EventHandles {
            device_id: self.device.id.clone(),
            battery_level: self.characteristics.battery_level.id.clone(),
            last_event: self.characteristics.event.id.clone(),
            facet: self.characteristics.facet.id.clone(),
            double_tap: self.characteristics.double_tap.id.clone(),
        };

        Ok(self
            .session
            .device_event_stream(&self.device.id)
            .await?
            .map(move |bt_event| gatt::Event::from_bluetooth_event(bt_event, &handles))
            .filter_map(|res| async move {
                match res {
                    Ok(event) => Some(event),
                    Err(e) => {
                        log::warn!("failed to decode event in stream: {e}");
                        None
                    }
                }
            })
            .boxed())
    }
}
