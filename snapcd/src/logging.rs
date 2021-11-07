/// Sets up the SQLite callback to log using the [`tracing`] crate.
///
/// # Safety
///
/// Must not be run at the same time as any SQLite commands are running.
pub unsafe fn setup_sqlite_callback() {
    static ONCE: std::sync::Once = std::sync::Once::new();

    fn sqlite_logging_callback(err_code: i32, err_msg: &str) {
        tracing::warn!("sqlite error {}: {}", err_code, err_msg);
    }

    // This is unsafe because it is not thread safe ("No other SQLite calls may be made while
    // config_log is running, and multiple threads may not call config_log simultaneously.")
    // as well sqlite_logging_callback having the requirements that they do not invoke SQLite,
    // and must be thread safe itself.
    ONCE.call_once(|| {
        rusqlite::trace::config_log(Some(sqlite_logging_callback))
            .expect("failed to set up logging")
    });
}

pub fn setup_logging(level: u64) {
    use tracing_subscriber::filter::LevelFilter;

    let filter = match level {
        0 => LevelFilter::WARN,
        1 => LevelFilter::INFO,
        2 => LevelFilter::DEBUG,
        3..=std::u64::MAX => LevelFilter::TRACE,
    };

    tracing_log::LogTracer::init().expect("failed to set tracing-log log subscriber");

    let collector = tracing_subscriber::fmt().with_max_level(filter).finish();

    tracing::subscriber::set_global_default(collector).expect("failed to set tracing subscriber");
}
