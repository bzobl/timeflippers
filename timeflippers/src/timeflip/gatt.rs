//! Low level types for communicating with TimeFlip2 using BLE/GATT
#![deny(missing_docs)]

use bluez_async::{
    uuid_from_u16, BluetoothError, BluetoothEvent, BluetoothSession, CharacteristicEvent,
    CharacteristicId, CharacteristicInfo, DeviceEvent, DeviceId,
};
use bytes::{Buf, BufMut};
use chrono::NaiveDateTime;
use std::{convert::Infallible, fmt, num::TryFromIntError, string::FromUtf8Error, time::Duration};
use thiserror::Error;
use uuid::Uuid;

/// A GATT service.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Service {
    /// GATT Battery service.
    Battery,
    /// TimeFlip service.
    TimeFlip,
}

impl Service {
    /// The UUID of the service.
    pub fn uuid(&self) -> Uuid {
        use Service::*;

        match self {
            Battery => uuid_from_u16(0x180F),
            TimeFlip => "F1196F50-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
        }
    }
}

/// A GATT characteristic belonging to a [Service]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Characteristic {
    /// The TimeFlip2's battery level in percent.
    ///
    /// Supports Read and Notify.
    BatteryLevel,
    /// The most current event in TimeFlip2's event log.
    ///
    /// Saved as ASCII text.
    ///
    /// Supports Read and Notify.
    Event,
    /// The facet currently pointing upward.
    ///
    /// Supports Read and Notify.
    Facet,
    /// Result of command initiated through [Characteristic::Command].
    ///
    /// Can only be read from.
    CommandResult,
    /// Characteristic to send [commands](Command) to the TimeFlip2.
    ///
    /// Supports Write and Read. When reading this characteristic, the last executed command's
    /// success state is returned. The command's output can be read from
    /// [Characteristic::CommandResult].
    Command,
    /// Characteristic to notify about double taps.
    ///
    /// Supports Notify only.
    DoubleTap,
    /// System state for TimeFlip2 calibration/initialization.
    ///
    /// Supports Read and Notify.
    SystemState,
    /// TimeFlip2 requires the password written to this characeristic.
    ///
    /// The password is reset whenever the bluetooth connection is disrupted. If no or
    /// the wrong password is set, TimeFlip2 will refuse to execute some commands and set
    /// [Characteristic::CommandResult] accordingly.
    ///
    /// Can only be written to.
    Password,
    /// Characteristic to read history of flip events from the TimeFlip2's memory.
    ///
    /// Supports Write, Read and Notify.
    History,
}

impl Characteristic {
    /// The [Service] this characteristic belongs to.
    pub fn service(&self) -> Service {
        use Characteristic::*;

        match self {
            BatteryLevel => Service::Battery,
            Event | Facet | CommandResult | Command | DoubleTap | SystemState | Password
            | History => Service::TimeFlip,
        }
    }

    /// The UUID of the characteristic.
    pub fn uuid(&self) -> Uuid {
        use Characteristic::*;

        match self {
            BatteryLevel => uuid_from_u16(0x2A19),
            Event => "F1196F51-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            Facet => "F1196F52-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            CommandResult => "F1196F53-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            Command => "F1196F54-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            DoubleTap => "F1196F55-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            SystemState => "F1196F56-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            Password => "F1196F57-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
            History => "F1196F58-71A4-11E6-BDF4-0800200C9A66"
                .parse()
                .expect("is a UUID"),
        }
    }

    /// Query the characteristic's handle to be used by bluez.
    pub async fn get_info(
        &self,
        session: &BluetoothSession,
        device: &DeviceId,
    ) -> Result<CharacteristicInfo, BluetoothError> {
        session
            .get_service_characteristic_by_uuid(device, self.service().uuid(), self.uuid())
            .await
    }
}

/// A command sent to the TimeFlip's [Characteristic::Command] characteristic.
pub enum Command {
    /// Set TimeFlip2's lock mode.
    ///
    /// In this mode the dice “freezes” to count time on the last active facet and blocks
    /// the device from switching facets when it is flipped.
    LockMode(bool),
    /// Set auto-pause time in minutes. 0 disables auto-pause.
    AutoPauseTime(super::Minutes),
    /// Set TimeFlip2's pause mode.
    PauseMode(bool),
    /// Get current time saved on TimeFlip2.
    GetTime,
    /// Set the time (in UTC) saved on TimeFlip2.
    Time(NaiveDateTime),
    /// Set The LED brightness level in percent.
    Brightness(super::Percent),
    /// Set the Led Blink interval in seconds (range 5-60).
    BlinkInterval(super::BlinkInterval),
    /// Read TimeFlip2's [SystemStatus].
    ReadStatus,
    /// Set the color of a facet.
    SetColor {
        facet: super::Facet,
        red: u16,
        green: u16,
        blue: u16,
    },
    /// Set the task parameters of a facet.
    SetTaskParameter(super::Facet, FacetTask),
    /// Get the task parameter of a facet.
    GetTaskParameter(super::Facet),
    // missing: Name Record (0x15, no idea what this actually does),
    //          Set double-tap (0x16), Read double-tap (0x17), set password (0x30),
    //          reset tasks (0xFE), factory reset (0xFF)
}

impl Command {
    /// Get the command's id
    pub fn id(&self) -> u8 {
        use Command::*;
        match self {
            LockMode(_) => 0x04,
            AutoPauseTime(_) => 0x05,
            PauseMode(_) => 0x06,
            GetTime => 0x07,
            Time(_) => 0x08,
            Brightness(_) => 0x09,
            BlinkInterval(_) => 0x0A,
            ReadStatus => 0x10,
            SetColor { .. } => 0x11,
            SetTaskParameter(_, _) => 0x13,
            GetTaskParameter(_) => 0x14,
        }
    }

    /// Get the command's data to write to the GATT characteristic.
    pub fn to_vec(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(3);
        data.put_u8(self.id());

        use Command::*;
        match self {
            LockMode(on) | PauseMode(on) => {
                if *on {
                    data.put_u8(0x01)
                } else {
                    data.put_u8(0x02)
                }
            }
            AutoPauseTime(time) => data.put_u16(time.0),
            Time(time) => {
                data.put_u64(u64::try_from(time.timestamp()).expect("timestamp is positive"))
            }
            Brightness(super::Percent(value)) | BlinkInterval(super::BlinkInterval(value)) => {
                data.put_u8(*value)
            }
            GetTime | ReadStatus => {}
            SetColor {
                facet,
                red,
                green,
                blue,
            } => {
                data.put_u8(facet.0);
                data.put_u16(*red);
                data.put_u16(*green);
                data.put_u16(*blue);
            }
            SetTaskParameter(facet, task) => {
                data.put_u8(facet.0);
                match task {
                    FacetTask::Simple => {
                        data.put_u8(0);
                        data.put_u32(0);
                    }
                    FacetTask::Pomodoro(timer) => {
                        data.put_u8(1);
                        data.put_u32(*timer);
                    }
                }
            }
            GetTaskParameter(facet) => data.put_u8(facet.0),
        }
        data
    }
}

/// Trait for the output of a command read via [Characteristic::CommandResult].
pub trait CommandResult {
    type Output;
    type Error;

    /// Construct an object from the data read from [Characteristic::CommandResult].
    fn from_data(data: &[u8]) -> Result<Self::Output, Self::Error>;
}

impl CommandResult for () {
    type Output = Self;
    type Error = Infallible;

    fn from_data(_data: &[u8]) -> Result<Self::Output, Self::Error> {
        Ok(())
    }
}

/// Error for converting a [Characteristic::CommandResult]'s output to [NaiveDateTime].
#[derive(Debug, Error)]
pub enum GetTimeError {
    #[error("need 9 byte for timestamp, read {0}")]
    TooShort(usize),
    #[error("invalid command in result: {0}")]
    InvalidCommand(u8),
    #[error("cannot convert timestamp: {0}")]
    TimestampTooBig(#[from] TryFromIntError),
    #[error("timestamp {0} not representable")]
    Timestamp(u64),
}

impl CommandResult for NaiveDateTime {
    type Output = Self;
    type Error = GetTimeError;

    fn from_data(mut data: &[u8]) -> Result<Self::Output, Self::Error> {
        if data.len() < 9 {
            return Err(GetTimeError::TooShort(data.len()));
        }
        let cmd = data.get_u8();
        if cmd != 0x07 {
            return Err(GetTimeError::InvalidCommand(cmd));
        }

        let timestamp = data.get_u64();
        NaiveDateTime::from_timestamp_opt(timestamp.try_into()?, 0)
            .ok_or(GetTimeError::Timestamp(timestamp))
    }
}

/// Error for converting a [Characteristic::CommandResult]'s output to [SystemStatus].
#[derive(Debug, Error)]
pub enum SystemStatusError {
    #[error("system status needs 4 bytes, read {0}")]
    TooShort(usize),
    #[error("unhandled lock mode value: 0x{0:X}")]
    InvalidLockMode(u8),
    #[error("unhandled pause mode value: 0x{0:X}")]
    InvalidPauseMode(u8),
}

/// The system status of TimeFlip2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemStatus {
    /// Whether the TimeFlip2 is in lock mode.
    pub lock_mode: bool,
    /// Whether the TimeFlip2 is in pause mode.
    pub pause_mode: bool,
    /// Time in minutes after which time tracking is paused automatically.
    pub auto_pause_time: super::Minutes,
}

impl CommandResult for SystemStatus {
    type Output = Self;
    type Error = SystemStatusError;

    fn from_data(mut data: &[u8]) -> Result<Self, SystemStatusError> {
        if data.len() < 4 {
            return Err(SystemStatusError::TooShort(data.len()));
        }
        let lock_mode = data.get_u8();
        let pause_mode = data.get_u8();
        let auto_pause_time_minutes = data.get_u16();

        Ok(SystemStatus {
            lock_mode: match lock_mode {
                1 => true,
                2 => false,
                v => return Err(SystemStatusError::InvalidLockMode(v)),
            },
            pause_mode: match pause_mode {
                1 => true,
                2 => false,
                v => return Err(SystemStatusError::InvalidPauseMode(v)),
            },
            auto_pause_time: super::Minutes(auto_pause_time_minutes),
        })
    }
}

/// Task assigned to a facet.
#[derive(Debug, PartialEq, Eq)]
pub enum FacetTask {
    /// Simple counting up timer.
    Simple,
    /// Pomodoro timer with limit in seconds.
    Pomodoro(u32),
}

/// Error for converting a [Characteristic::CommandResult]'s output to [FacetSettings].
#[derive(Debug, Error)]
pub enum FacetSettingsError {
    #[error("system status needs 4 bytes, read {0}")]
    TooShort(usize),
    #[error("invalid command in response: 0x{0:X}")]
    InvalidCommand(u8),
    #[error("unhandled task value: 0x{0:X}")]
    InvalidTask(u8),
    #[error("{0}")]
    InvalidFacet(#[from] super::FacetError),
}

/// Settings of a facet of the TimeFlip2
#[derive(Debug, PartialEq, Eq)]
pub struct FacetSettings {
    /// The facet.
    pub facet: super::Facet,
    /// The assigned task.
    pub task: FacetTask,
    /// The number of seconds from the moment the timer was started.
    pub seconds_since_start: u32,
}

impl CommandResult for FacetSettings {
    type Output = Self;
    type Error = FacetSettingsError;

    /// Construct a [FacetTask] from the data read from [Characteristic::CommandResult].
    fn from_data(mut data: &[u8]) -> Result<Self, FacetSettingsError> {
        if data.len() < 11 {
            return Err(FacetSettingsError::TooShort(data.len()));
        }
        let cmd = data.get_u8();
        let facet = data.get_u8();
        let task = data.get_u8();
        let timer_seconds = data.get_u32();
        let seconds_since_start = data.get_u32();

        if cmd != 0x14 {
            return Err(FacetSettingsError::InvalidCommand(cmd));
        }
        let facet = super::Facet::new(facet)?;
        let task = match task {
            0 => FacetTask::Simple,
            1 => FacetTask::Pomodoro(timer_seconds),
            _ => return Err(FacetSettingsError::InvalidTask(task)),
        };

        Ok(FacetSettings {
            facet,
            task,
            seconds_since_start,
        })
    }
}

/// Error for converting a [Characteristic::CommandResult]'s output to [SyncState].
#[derive(Debug, Error)]
pub enum SyncStateError {
    #[error("sync state needs 4 bytes, read {0}")]
    TooShort(usize),
    #[error("unhandled sync type: 0x{0:X}, 0x{1:X}")]
    InvalidSyncType(u8, u8),
    #[error("unhandled hardware error: 0x{0:X}, 0x{1:X}")]
    InvalidHardwareError(u8, u8),
}

/// Indicates that some type of synchronization is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncType {
    /// The device is synchronized.
    Synchronized,
    /// The device has been reset to the factory settings.
    FactoryReset,
    /// Time synchronization required, see [Command::Time].
    Time,
    /// Facet color synchronization required, see [Command::SetColor].
    FacetColor,
    /// LED brightness synchronization required, see [Command::Brightness].
    LedBrightness,
    /// Blink interval synchronization required, see [Command::BlinkInterval].
    BlinkInterval,
    /// Task parameters synchronization required, see [Command::SetTaskParameter].
    TaskParameters,
    /// Auto-pause synchronization required, see [Command::AutoPauseTime].
    AutoPause,
}

/// Synchronization state used to keep the application and the TimeFlip2 up-to-date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    /// The synchronization state.
    sync: SyncType,
    /// Detected accelerometer error.
    accelerometer_error: bool,
    /// Detected flash error.
    flash_error: bool,
}

impl SyncState {
    /// Construct a [SyncState] from the data read from [Characteristic::CommandResult].
    pub fn from_data(data: &[u8]) -> Result<Self, SyncStateError> {
        if data.len() < 4 {
            return Err(SyncStateError::TooShort(data.len()));
        }
        let sync = match (data[0], data[1]) {
            (0, 0) => SyncType::Synchronized,
            (1, 0) => SyncType::FactoryReset,
            (2, 1) => SyncType::Time,
            (2, 2) => SyncType::FacetColor,
            (2, 3) => SyncType::LedBrightness,
            (2, 4) => SyncType::BlinkInterval,
            (2, 5) => SyncType::TaskParameters,
            (2, 6) => SyncType::AutoPause,
            (x, y) => return Err(SyncStateError::InvalidSyncType(x, y)),
        };

        let (accelerometer_error, flash_error) = match (data[2], data[3]) {
            (0, 0) => (false, false),
            (2, 1) => (true, false),
            (2, 2) => (false, true),
            (2, 3) => (true, true),
            (x, y) => return Err(SyncStateError::InvalidHardwareError(x, y)),
        };

        Ok(SyncState {
            sync,
            accelerometer_error,
            flash_error,
        })
    }
}

/// Error when parsing a history entry.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum EntryError {
    #[error("end of history")]
    EndOfHistory,
    #[error("too short")]
    TooShort,
    #[error("invalid facet: {0}")]
    InvalidFacet(#[from] super::FacetError),
    #[error("invalid start time of flip: {0}")]
    InvalidTimestamp(u64),
}

/// An entry from TimeFlip2's history.
#[derive(Debug, Clone)]
pub struct Entry {
    /// ID of the entry.
    pub id: u32,
    /// Active facet.
    pub facet: super::Facet,
    /// Whether or not the face is in pause state.
    pub pause: bool,
    /// The time the dice was flipped.
    pub time: NaiveDateTime,
    /// Duration the facet was active.
    pub duration: Duration,
}

impl Entry {
    /// Construct a [Entry] from the data read from [Characteristic::History].
    pub fn from_data(mut data: &[u8]) -> Result<Entry, EntryError> {
        if data.len() < 17 {
            return Err(EntryError::TooShort);
        }
        let id = data.get_u32();
        let facet = data.get_u8();
        let start_time = data.get_u64();
        let duration = data.get_u32();

        if id == 0 && facet == 0 && start_time == 0 && duration == 0 {
            return Err(EntryError::EndOfHistory);
        }

        let (facet, pause) = if facet > 127 {
            (facet - 128, true)
        } else {
            (facet, false)
        };

        Ok(Entry {
            id,
            facet: super::Facet::new(facet)?,
            pause,
            time: NaiveDateTime::from_timestamp_opt(
                start_time
                    .try_into()
                    .map_err(|_| EntryError::InvalidTimestamp(start_time))?,
                0,
            )
            .ok_or(EntryError::InvalidTimestamp(start_time))?,
            duration: Duration::from_secs(duration.into()),
        })
    }
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} {} on {} for {} seconds",
            self.id,
            self.facet,
            if self.pause { "paused" } else { "started" },
            self.time,
            self.duration.as_secs()
        )
    }
}

/// Error while decoding bluetooth event
#[derive(Debug, Error)]
pub enum EventError {
    #[error("unexpected bluetooth event stream: {0:?}")]
    UnexpectedEvent(BluetoothEvent),
    #[error("event for unexpected device in stream: {0:?}")]
    UnexpectedDevice(DeviceId),
    #[error("event for unexpected characteristic in stream: {0:?}")]
    UnexpectedCharacteristic(CharacteristicId),
    #[error("ignored connected event")]
    IgnoreConnected,
    #[error("value too short for {0}")]
    TooShort(String),
    #[error("{0}")]
    BatteryLevel(#[from] super::PercentError),
    #[error("{0}")]
    Event(#[from] FromUtf8Error),
    #[error("{0}")]
    Facet(#[from] super::FacetError),
    #[error("invalid facet in double tap event: {0}")]
    DoubleTap(super::FacetError),
}

/// Bluez handles for identifying Bluetooth events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHandles {
    pub device_id: DeviceId,
    pub battery_level: CharacteristicId,
    pub last_event: CharacteristicId,
    pub facet: CharacteristicId,
    pub double_tap: CharacteristicId,
}

/// Events for subscribed properties of the TimeFlip2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Device has disconnected.
    Disconnected,
    /// Battery level has changed.
    BatteryLevel(super::Percent),
    /// Status message has changed.
    Event(String),
    /// The facet has changed.
    Facet(super::Facet),
    /// Double Tap / Pause detected.
    ///
    /// This event indicates that the TimeFlip2 has been set into pause mode either
    /// by double-tapping or by auto-pause.
    DoubleTap {
        /// The facet currently facing up.
        facet: super::Facet,
        /// Whether pause mode has been entered or left.
        pause: bool,
    },
}

impl Event {
    /// Construct an [Event] from a [BluetoothEvent].
    pub fn from_bluetooth_event(
        bt_event: BluetoothEvent,
        handles: &EventHandles,
    ) -> Result<Self, EventError> {
        match bt_event {
            BluetoothEvent::Characteristic {
                id,
                event: CharacteristicEvent::Value { value },
            } => {
                if id == handles.battery_level {
                    log::debug!("Battery Level event");
                    value
                        .first()
                        .ok_or(EventError::TooShort("Battery Level".into()))
                        .and_then(|v| super::Percent::new(*v).map_err(Into::into))
                        .map(Event::BatteryLevel)
                } else if id == handles.last_event {
                    log::debug!("Eventlog event");
                    String::from_utf8(value)
                        .map_err(Into::into)
                        .map(Event::Event)
                } else if id == handles.facet {
                    log::debug!("Facet event");
                    value
                        .first()
                        .ok_or(EventError::TooShort("Facet".into()))
                        .and_then(|v| super::Facet::new(*v).map_err(Into::into))
                        .map(Event::Facet)
                } else if id == handles.double_tap {
                    log::debug!("DoubleTap event");
                    value
                        .first()
                        .ok_or(EventError::TooShort("Double Tap".into()))
                        .and_then(|v| {
                            let (facet, pause) = if *v > 127 {
                                (*v - 128, true)
                            } else {
                                (*v, false)
                            };
                            super::Facet::new(facet)
                                .map(|facet| Event::DoubleTap { facet, pause })
                                .map_err(EventError::DoubleTap)
                        })
                } else {
                    Err(EventError::UnexpectedCharacteristic(id))
                }
            }
            BluetoothEvent::Device {
                id,
                event: DeviceEvent::Connected { connected },
            } => {
                if id != handles.device_id {
                    Err(EventError::UnexpectedDevice(id))
                } else if connected {
                    Err(EventError::IgnoreConnected)
                } else {
                    Ok(Event::Disconnected)
                }
            }
            BluetoothEvent::Adapter { .. }
            | BluetoothEvent::Device { .. }
            | BluetoothEvent::Characteristic { .. } => {
                // The adpter/device/characteristic events are marked as non-exhaustive, hence
                // we have to have a catch all here.
                Err(EventError::UnexpectedEvent(bt_event))
            }
        }
    }
}
