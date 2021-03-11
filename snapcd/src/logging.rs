/// Sets up the SQLite callback to log using the `log` crate.
///
/// # Safety
///
/// Must not be run at the same time as any SQLite commands are running.
pub unsafe fn setup_sqlite_callback() {
    static ONCE: std::sync::Once = std::sync::Once::new();

    fn sqlite_logging_callback(err_code: i32, err_msg: &str) {
        log::warn!("sqlite error {}: {}", err_code, err_msg);
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
    use simplelog::{LevelFilter, TerminalMode};

    let filter = match level {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        3..=std::u64::MAX => LevelFilter::Trace,
    };

    let log_config = simplelog::ConfigBuilder::new()
        .set_time_level(LevelFilter::Debug)
        .set_time_to_local(true)
        .build();

    match simplelog::TermLogger::init(filter, log_config, TerminalMode::Stderr) {
        Ok(()) => {}
        Err(err) => {
            panic!("{}: logger has been already set, this is a bug.", err)
        }
    }
}
