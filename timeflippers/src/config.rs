use crate::types::{BlinkInterval, Color, FacetTask, Minutes, Percent};
use std::default::Default;

/// Configuration of a TimeFlip2 facet.
#[derive(Debug, PartialEq, Eq)]
pub struct Side {
    /// The name of the facet.
    pub name: Option<String>,
    /// The color of the facet.
    pub color: Color,
    /// The task assigned to the facet.
    pub task: FacetTask,
}

impl Default for Side {
    fn default() -> Self {
        Side {
            name: None,
            color: Color::from_rgb(0, 0, 0),
            task: FacetTask::Simple,
        }
    }
}

/// Configuration of a TimeFlip2.
#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    /// The password to access the TimeFlip2.
    pub password: [u8; 6],
    /// Brightness of the TimeFlip2's LED.
    pub brightness: Percent,
    /// Blink interval of the TimeFlip2's LED, when not paused.
    pub blink_interval: BlinkInterval,
    /// Time after which activity is automatically paused.
    pub auto_pause: Minutes,
    /// Configuration for each facet/side.
    pub sides: [Side; 12],
}

impl Default for Config {
    fn default() -> Self {
        Config {
            password: [0x30; 6],
            brightness: Percent::new(100).expect("is a valid value"),
            blink_interval: BlinkInterval::new(30).expect("is a valid value"),
            auto_pause: Minutes(8 * 60),
            sides: Default::default(),
        }
    }
}
