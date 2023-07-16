use crate::types::{BlinkInterval, Color, Facet, FacetError, FacetTask, Minutes, Percent};
use serde::{
    de::{self, Error},
    Deserialize,
};
use std::default::Default;
use thiserror::Error as ThisError;

/// Configuration of a TimeFlip2 facet.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct Side {
    /// The name of the facet.
    pub facet: Facet,
    /// The name of the facet.
    pub name: Option<String>,
    /// The color of the facet.
    pub color: Color,
    /// The task assigned to the facet.
    pub task: FacetTask,
}

impl Side {
    /// Construct a side with default values.
    fn default(index: usize) -> Result<Side, FacetError> {
        Ok(Side {
            facet: Facet::new(index)?,
            name: None,
            color: Color::default(),
            task: FacetTask::Simple,
        })
    }
}

/// Configuration of a TimeFlip2.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename = "Timeflip")]
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
    #[serde(deserialize_with = "deserialize_sides")]
    pub sides: [Side; 12],
}

impl Default for Config {
    fn default() -> Self {
        Config {
            password: [0x30; 6],
            brightness: Percent::new(100).expect("is a valid value"),
            blink_interval: BlinkInterval::new(30).expect("is a valid value"),
            auto_pause: Minutes(8 * 60),
            sides: sides_from_vec(vec![]).expect("cannot fail"),
        }
    }
}

#[derive(Debug, ThisError)]
enum ExpectedSides {
    #[error("too many sides ({0}), up to 12 sides supported")]
    TooMany(usize),
    #[error("passed vector contains duplicates")]
    Duplicates,
}

fn sides_from_vec(mut sides: Vec<Side>) -> Result<[Side; 12], ExpectedSides> {
    if sides.len() > 12 {
        return Err(ExpectedSides::TooMany(sides.len()));
    }

    let indices: Vec<usize> = sides.iter().map(|s| usize::from(s.facet.index())).collect();
    for i in 1..=12 {
        if !indices.contains(&i) {
            sides.push(Side::default(i).expect("is in range"))
        }
    }

    if sides.len() != 12 {
        return Err(ExpectedSides::Duplicates);
    }

    sides.sort_by(|a, b| a.facet.cmp(&b.facet));
    Ok(sides.try_into().expect("is 12 elements long"))
}

fn deserialize_sides<'de, D>(deserializer: D) -> Result<[Side; 12], D::Error>
where
    D: de::Deserializer<'de>,
{
    let sides = Vec::<Side>::deserialize(deserializer)?;
    sides_from_vec(sides).map_err(Error::custom)
}
