use crate::errors::*;
use enum_iterator::IntoEnumIterator;
use std::str::FromStr;
use teloxide::utils::command::BotCommand;

#[derive(Debug, BotCommand, Clone)]
#[command(rename = "lowercase")]
pub enum Command {
    Start,
    Help,
}

#[derive(Debug, Clone, Copy)]
pub enum MediaType {
    Image,
    Video,
}

#[derive(Debug, IntoEnumIterator, PartialEq, Clone, Copy)]
pub enum ConvertSize {
    Small,
    Medium,
    Large,
}
#[derive(Debug, IntoEnumIterator, PartialEq, Clone, Copy)]
pub enum ConvertPosition {
    Left,
    Center,
    Right,
}
#[derive(Debug, Clone, Copy)]
pub struct ConvertConfig {
    pub size: ConvertSize,
    pub position: Option<ConvertPosition>,
}

impl ConvertSize {
    pub fn resize(&self, width: u32, height: u32) -> (u32, u32) {
        let t_width = 512;
        let t_height = match self {
            Self::Small => 128,
            Self::Medium => 256,
            Self::Large => 512,
        };
        let n_width = ((width * t_height) as f32 / height as f32) as u32;
        let n_height = ((height * t_width) as f32 / width as f32) as u32;
        if n_width <= t_width {
            (n_width, t_height)
        } else {
            (t_width, n_height)
        }
    }
}
impl ToString for ConvertSize {
    fn to_string(&self) -> String {
        match self {
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
        }
        .to_string()
    }
}
impl FromStr for ConvertSize {
    type Err = ConfigError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Small" => Ok(Self::Small),
            "Medium" => Ok(Self::Medium),
            "Large" => Ok(Self::Large),
            _ => Err(ConfigError::Parse(s.to_owned())),
        }
    }
}
impl ToString for ConvertPosition {
    fn to_string(&self) -> String {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
        }
        .to_string()
    }
}
impl FromStr for ConvertPosition {
    type Err = ConfigError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(Self::Left),
            "Center" => Ok(Self::Center),
            "Right" => Ok(Self::Right),
            _ => Err(ConfigError::Parse(s.to_owned())),
        }
    }
}
impl From<(u32, u32)> for ConvertConfig {
    fn from((width, height): (u32, u32)) -> Self {
        if width > 384 || height > 256 {
            Self {
                size: ConvertSize::Large,
                position: None,
            }
        } else if height > 128 {
            Self {
                size: ConvertSize::Medium,
                position: Some(ConvertPosition::Center),
            }
        } else {
            Self {
                size: ConvertSize::Small,
                position: Some(ConvertPosition::Center),
            }
        }
    }
}
