pub fn init() {
    #[cfg(debug_assertions)]
    {
        use std::io::IsTerminal;

        let _ = tracing_log::LogTracer::init();

        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

        let _ = tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_ansi(std::io::stderr().is_terminal())
            .compact()
            .try_init();
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn both_log_facades_follow_the_build_profile() {
        if cfg!(debug_assertions) {
            assert_eq!(
                tracing::level_filters::STATIC_MAX_LEVEL,
                tracing::level_filters::LevelFilter::TRACE
            );

            assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Trace);
        } else {
            assert_eq!(
                tracing::level_filters::STATIC_MAX_LEVEL,
                tracing::level_filters::LevelFilter::OFF
            );

            assert_eq!(log::STATIC_MAX_LEVEL, log::LevelFilter::Off);
        }
    }
}
