use serde::{
    de::{self, Error},
    Deserialize,
};
use std::{default::Default, fmt};
use thiserror::Error;

/// Error constructing a [Percent] object.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum PercentError {
    #[error("{0} out of range (0-100%)")]
    OutOfRange(usize),
}

/// Representation of a value in percent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Percent(u8);

impl Percent {
    /// Construct a [Percent] object.
    pub fn new(percent: usize) -> Result<Self, PercentError> {
        if percent <= 100 {
            Ok(Percent(u8::try_from(percent).expect("checked above")))
        } else {
            Err(PercentError::OutOfRange(percent))
        }
    }

    /// Get the value as integer.
    pub fn get(&self) -> u8 {
        self.0
    }
}

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}%", self.0)
    }
}

impl TryFrom<usize> for Percent {
    type Error = PercentError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Percent::new(value)
    }
}

impl<'de> de::Deserialize<'de> for Percent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let v = usize::deserialize(deserializer)?;
        Percent::new(v).map_err(D::Error::custom)
    }
}

/// A type representing minutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minutes(pub u16);

impl fmt::Display for Minutes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} minutes", self.0)
    }
}

impl<'de> de::Deserialize<'de> for Minutes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let v = u16::deserialize(deserializer)?;
        Ok(Minutes(v))
    }
}

/// Representation of the color of the LED
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Color {
    red: u16,
    green: u16,
    blue: u16,
}

impl Color {
    /// Construct the color from its RGB value.
    pub fn from_rgb(red: u16, green: u16, blue: u16) -> Self {
        Color { red, green, blue }
    }

    /// Get the Colors RGB value.
    pub fn rgb(&self) -> (u16, u16, u16) {
        (self.red, self.green, self.blue)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (r, g, b) = self.rgb();
        write!(f, "RGB({r},{g},{b})")
    }
}

/// Error while constructing a [Facet].
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum FacetError {
    #[error("invalid facet index {0}")]
    InvalidIndex(usize),
}

/// The side of a TimeFlip2.
#[derive(Debug, Clone, Ord, PartialEq, PartialOrd, Eq)]
pub struct Facet(u8);

impl Facet {
    /// Construct a [Facet].
    ///
    /// The facets are indexed from 1 to 12, inclusive.
    pub fn new(index: usize) -> Result<Self, FacetError> {
        if index >= 1 || index <= 12 {
            Ok(Facet(u8::try_from(index).expect("in range")))
        } else {
            Err(FacetError::InvalidIndex(index))
        }
    }

    /// Get the index of the facet.
    ///
    /// Please note that TimeFlip uses one-based indices.
    pub fn index(&self) -> u8 {
        self.0
    }

    /// Get the zero based index of a facet.
    pub fn index_zero(&self) -> usize {
        usize::from(self.index()) - 1
    }
}

impl fmt::Display for Facet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Facet({})", self.0)
    }
}

impl<'de> de::Deserialize<'de> for Facet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let v = usize::deserialize(deserializer)?;
        Facet::new(v).map_err(D::Error::custom)
    }
}

/// Task assigned to a facet.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum FacetTask {
    /// Simple counting up timer.
    Simple,
    /// Pomodoro timer with limit in seconds.
    Pomodoro(u32),
}

impl fmt::Display for FacetTask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FacetTask::Simple => write!(f, "Simple"),
            FacetTask::Pomodoro(s) => write!(f, "Pomodoro Timer ({s} seconds)"),
        }
    }
}

/// Error constructing a [BlinkInterval] object.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum BlinkIntervalError {
    #[error("{0} out of range (5-60 seconds)")]
    OutOfRange(usize),
}

/// Interval in seconds in which the LED of the TimeFlip2 will blink when active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlinkInterval(u8);

impl BlinkInterval {
    /// Construct a new [BlinkInterval] object.
    ///
    /// The interval value is given as seconds in range 5 to 60, inclusive.
    pub fn new(seconds: usize) -> Result<Self, BlinkIntervalError> {
        if (5..=60).contains(&seconds) {
            Ok(BlinkInterval(u8::try_from(seconds).expect("in range")))
        } else {
            Err(BlinkIntervalError::OutOfRange(seconds))
        }
    }

    /// Get the blink interval in seconds.
    pub fn seconds(&self) -> u8 {
        self.0
    }
}

impl fmt::Display for BlinkInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} seconds", self.0)
    }
}

impl<'de> de::Deserialize<'de> for BlinkInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let v = usize::deserialize(deserializer)?;
        BlinkInterval::new(v).map_err(D::Error::custom)
    }
}
