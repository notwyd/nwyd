#[cfg(feature = "analytics")]
mod enabled;
#[cfg(feature = "analytics")]
pub use enabled::{Analytics, Config, DEFAULT_BASE_URL, DEFAULT_ENVIRONMENT_KEY, configured};

#[cfg(not(feature = "analytics"))]
mod disabled;
#[cfg(not(feature = "analytics"))]
pub use disabled::Analytics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: &'static str,
    pub recipient: String,
    pub text: String,
}
