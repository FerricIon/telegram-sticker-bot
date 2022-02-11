use thiserror::Error;
use ubyte::ByteUnit;

#[derive(Debug, Error)]
pub enum CallbackError {
    #[error("Unknown callback: {0}.")]
    Unknown(String),
    #[error("Incompatible callback.")]
    Incompatible,
}

#[derive(Debug, Error)]
pub enum ConvertError {
    #[error("Duration too long: {0:.3}s exceeds 3s.")]
    Duration(f32),
    #[error("File size too large: {:.3} exceeds 5MiB.", ByteUnit::Byte(*.0))]
    FileSize(u64),
    #[error("Failed to read the video's {0} from \"{1}\".")]
    Format(String, String),
    #[error("Invalid media type.")]
    MediaType,
    #[error("Internal error: {0}")]
    Internal(anyhow::Error),
}
impl ConvertError {
    pub fn wrap<E: Into<anyhow::Error>>(e: E) -> Self {
        match e.into().downcast::<Self>() {
            Ok(e) => e,
            Err(e) => Self::Internal(e),
        }
    }
}

#[derive(PartialEq, Debug, Error, Clone)]
pub enum PropsError {
    #[error("Failed to parse props string: {0}.")]
    Parse(String),
    #[error("Failed to get props of the message.")]
    Message,
    #[error("Failed to get the original media.")]
    Origin,
}
