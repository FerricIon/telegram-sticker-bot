use crate::errors::*;
use std::{cmp::Ordering, fmt::Display, str::FromStr};
use teloxide::{types::InlineKeyboardButton, utils::command::BotCommand};

#[derive(Debug, BotCommand, Clone)]
#[command(rename = "lowercase")]
pub enum Command {
    Start,
    Help,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Callback {
    Small,
    Medium,
    Large,
    Left,
    Center,
    Right,
    SpeedUp,
}
#[derive(Debug)]
pub enum CallbackKind {
    Size,
    Position,
    Time,
}

#[derive(Debug, Clone, Copy)]
pub enum MediaType {
    Image,
    Video,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Alignment {
    Left,
    Center,
    Right,
}
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum LayoutProp {
    Small(Alignment),
    Medium(Alignment),
    Large,
}

#[derive(Debug, Clone, Copy)]
pub struct PlaybackProp {
    pub speed_up: bool,
}

impl Callback {
    pub fn kind(&self) -> CallbackKind {
        match self {
            Self::Small | Self::Medium | Self::Large => CallbackKind::Size,
            Self::Left | Self::Center | Self::Right => CallbackKind::Position,
            Self::SpeedUp => CallbackKind::Time,
        }
    }
}
impl FromStr for Callback {
    type Err = CallbackError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Small" => Ok(Self::Small),
            "Medium" => Ok(Self::Medium),
            "Large" => Ok(Self::Large),
            "Left" => Ok(Self::Left),
            "Center" => Ok(Self::Center),
            "Right" => Ok(Self::Right),
            "SpeedUp" => Ok(Self::SpeedUp),
            _ => Err(CallbackError::Unknown(s.to_owned())),
        }
    }
}
impl From<Alignment> for Callback {
    fn from(position: Alignment) -> Self {
        use Alignment::*;
        match position {
            Left => Self::Left,
            Center => Self::Center,
            Right => Self::Right,
        }
    }
}
impl From<Callback> for InlineKeyboardButton {
    fn from(callback: Callback) -> Self {
        use Callback::*;
        let (text, data) = match callback {
            Small => ("Small", "Small"),
            Medium => ("Medium", "Medium"),
            Large => ("Large", "Large"),
            Left => ("Left", "Left"),
            Center => ("Center", "Center"),
            Right => ("Right", "Right"),
            SpeedUp => ("Speed me up!", "SpeedUp"),
        };
        Self::callback(text.to_owned(), data.to_owned())
    }
}

impl Alignment {
    fn pad_x(&self, width: u32) -> u32 {
        match self {
            Self::Left => 0,
            Self::Center => (512 - width) / 2,
            Self::Right => 512 - width,
        }
    }
}
impl Display for Alignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Left => "Left",
                Self::Center => "Center",
                Self::Right => "Right",
            }
        )
    }
}
impl FromStr for Alignment {
    type Err = PropsError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Left" => Ok(Self::Left),
            "Center" => Ok(Self::Center),
            "Right" => Ok(Self::Right),
            _ => Err(PropsError::Parse(s.to_owned())),
        }
    }
}
impl TryFrom<Callback> for Alignment {
    type Error = CallbackError;
    fn try_from(callback: Callback) -> Result<Self, Self::Error> {
        use Callback::*;
        match callback {
            Left => Ok(Self::Left),
            Center => Ok(Self::Center),
            Right => Ok(Self::Right),
            _ => Err(CallbackError::Incompatible),
        }
    }
}
impl LayoutProp {
    pub fn resize(&self, width: u32, height: u32) -> (u32, u32, Option<u32>) {
        let b_width = 512;
        let b_height = match self {
            Self::Small(_) => 128,
            Self::Medium(_) => 256,
            Self::Large => 512,
        };
        let n_width = ((width * b_height) as f32 / height as f32) as u32;
        let n_height = ((height * b_width) as f32 / width as f32) as u32;

        let (width, height) = if n_width <= b_width {
            (n_width, b_height)
        } else {
            (b_width, n_height)
        };
        use Ordering::*;
        let x = match (width.cmp(&512), height.cmp(&512), self) {
            (Less, Less, Self::Small(p) | Self::Medium(p)) => Some(p.pad_x(width)),
            _ => None,
        };
        (b_width, b_height, x)
    }
    pub fn reset_size(self, s: Callback) -> Result<Self, CallbackError> {
        use Callback::*;
        match (self, s) {
            (Self::Small(p), Medium) => Ok(Self::Medium(p)),
            (Self::Medium(p), Small) => Ok(Self::Small(p)),
            (Self::Small(_) | Self::Medium(_), Large) => Ok(Self::Large),
            (Self::Large, Small) => Ok(Self::Small(Alignment::Center)),
            (Self::Large, Medium) => Ok(Self::Medium(Alignment::Center)),
            _ => Err(CallbackError::Incompatible),
        }
    }
    pub fn reset_alignment(self, s: Callback) -> Result<Self, CallbackError> {
        match self {
            Self::Small(_) => Ok(Self::Small(s.try_into()?)),
            Self::Medium(_) => Ok(Self::Medium(s.try_into()?)),
            Self::Large => Err(CallbackError::Incompatible),
        }
    }
}
impl Display for LayoutProp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Small(p) => format!("Small,{}", p),
                Self::Medium(p) => format!("Medium,{}", p),
                Self::Large => format!("Large,/"),
            }
        )
    }
}
impl FromStr for LayoutProp {
    type Err = PropsError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let arr: Vec<_> = s.split(',').collect();
        if arr.len() != 2 {
            return Err(PropsError::Parse(s.to_owned()));
        }
        match arr[0] {
            "Small" => Ok(Self::Small(arr[1].parse()?)),
            "Medium" => Ok(Self::Medium(arr[1].parse()?)),
            "Large" => Ok(Self::Large),
            _ => Err(PropsError::Parse(s.to_owned())),
        }
    }
}
impl From<(u32, u32)> for LayoutProp {
    fn from((width, height): (u32, u32)) -> Self {
        if width > 384 || height > 256 {
            Self::Large
        } else if height > 128 {
            Self::Medium(Alignment::Center)
        } else {
            Self::Small(Alignment::Center)
        }
    }
}
impl FromStr for PlaybackProp {
    type Err = PropsError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            speed_up: match s {
                "speed_up" => true,
                "original_speed" => false,
                _ => Err(PropsError::Parse(s.to_owned()))?,
            },
        })
    }
}
impl Display for PlaybackProp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            if self.speed_up {
                "speed_up"
            } else {
                "original_speed"
            }
        )
    }
}
