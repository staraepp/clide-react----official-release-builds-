use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("no microphone is available")]
    NoInputDevice,

    #[error("the microphone could not be opened: {0}")]
    DeviceUnavailable(String),

    #[error("microphone access has not been granted")]
    PermissionDenied,

    #[error("recording is already in progress")]
    AlreadyRecording,

    #[error("no recording is in progress")]
    NotRecording,

    #[error("nothing was recorded")]
    Empty,


    #[error("the recording could not be written: {0}")]
    Write(String),

    #[error("the audio engine stopped responding")]
    EngineGone,
}
