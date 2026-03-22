use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImeError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Dictionary error: {0}")]
    Dictionary(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Trie error: {0}")]
    Trie(String),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Platform error: {0}")]
    Platform(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Lock error: {0}")]
    Lock(String),
}

impl Clone for ImeError {
    fn clone(&self) -> Self {
        match self {
            ImeError::Config(s) => ImeError::Config(s.clone()),
            ImeError::Dictionary(s) => ImeError::Dictionary(s.clone()),
            ImeError::Io(s) => ImeError::Io(s.clone()),
            ImeError::Trie(s) => ImeError::Trie(s.clone()),
            ImeError::Pipeline(s) => ImeError::Pipeline(s.clone()),
            ImeError::Session(s) => ImeError::Session(s.clone()),
            ImeError::Platform(s) => ImeError::Platform(s.clone()),
            ImeError::Parse(s) => ImeError::Parse(s.clone()),
            ImeError::Lock(s) => ImeError::Lock(s.clone()),
        }
    }
}

impl From<std::io::Error> for ImeError {
    fn from(err: std::io::Error) -> Self {
        ImeError::Io(err.to_string())
    }
}

impl ImeError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn dictionary(msg: impl Into<String>) -> Self {
        Self::Dictionary(msg.into())
    }

    pub fn trie(msg: impl Into<String>) -> Self {
        Self::Trie(msg.into())
    }

    pub fn pipeline(msg: impl Into<String>) -> Self {
        Self::Pipeline(msg.into())
    }

    pub fn session(msg: impl Into<String>) -> Self {
        Self::Session(msg.into())
    }

    pub fn platform(msg: impl Into<String>) -> Self {
        Self::Platform(msg.into())
    }

    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }
}

pub type ImeResult<T> = Result<T, ImeError>;
