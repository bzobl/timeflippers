use std::fmt;
use thiserror::Error;

/// Error constructing a [Percent] object.
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum PercentError {
    #[error("{0} out of range (0-100%)")]
    OutOfRange(u8),
}

/// Representation of a value in percent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Percent(u8);

impl Percent {
    /// Construct a [Percent] object.
    pub fn new(percent: u8) -> Result<Self, PercentError> {
        if percent <= 100 {
            Ok(Percent(percent))
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

/// A type representing minutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minutes(pub u16);

impl fmt::Display for Minutes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}min", self.0)
    }
}

/// Representation of the color of the LED
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Color(u16, u16, u16);

impl Color {
    /// Construct the color from its RGB value.
    pub fn from_rgb(red: u16, green: u16, blue: u16) -> Self {
        Color(red, green, blue)
    }

    /// Get the Colors RGB value.
    pub fn rgb(&self) -> (u16, u16, u16) {
        (self.0, self.1, self.2)
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
    InvalidIndex(u8),
}

/// The side of a TimeFlip2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet(u8);

impl Facet {
    /// Construct a [Facet].
    ///
    /// The facets are indexed from 1 to 12, inclusive.
    pub fn new(index: u8) -> Result<Self, FacetError> {
        if index >= 1 || index <= 12 {
            Ok(Facet(index))
        } else {
            Err(FacetError::InvalidIndex(index))
        }
    }

    /// Get the index of the facet.
    pub fn index(&self) -> u8 {
        self.0
    }
}

impl fmt::Display for Facet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Facet({})", self.0)
    }
}

/// Task assigned to a facet.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    OutOfRange(u8),
}

/// Interval in seconds in which the LED of the TimeFlip2 will blink when active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlinkInterval(u8);

impl BlinkInterval {
    /// Construct a new [BlinkInterval] object.
    ///
    /// The interval value is given as seconds in range 5 to 60, inclusive.
    pub fn new(seconds: u8) -> Result<Self, BlinkIntervalError> {
        if (5..=60).contains(&seconds) {
            Ok(BlinkInterval(seconds))
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
        write!(f, "{}min", self.0)
    }
}
