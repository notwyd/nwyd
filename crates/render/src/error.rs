#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no GPU adapter available: {0}")]
    NoAdapter(String),
    #[error("could not open a GPU device: {0}")]
    NoDevice(String),
    #[error("adapter {adapter} is missing required features: {missing}")]
    MissingFeatures { adapter: String, missing: String },
}

pub type Result<T> = std::result::Result<T, Error>;
