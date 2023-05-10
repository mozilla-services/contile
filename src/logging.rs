//! Mozilla logging initialization and configuration

use std::io;

use crate::create_app_version;
use crate::error::HandlerResult;

use slog::{self, slog_o, Drain};
use slog_mozlog_json::MozLogJson;

/// Handle logging initialization.
///
/// This uses the `slog_mozlog` crate
/// to extend `slog` logging. The `json` argument flags if output should
/// be in JSON format (the default for production logging), or in
/// a more human readable form. For Contile, this is configured using
/// the `human_logs` setting (see [crate::settings::Settings])
pub fn init_logging(json: bool) -> HandlerResult<()> {
    let logger = if json {
        let hostname = gethostname::gethostname()
            .into_string()
            .expect("Couldn't get hostname");

        let drain = MozLogJson::new(io::stdout())
            .logger_name(create_app_version("-"))
            .msg_type(format!("{}:log", env!("CARGO_PKG_NAME")))
            .hostname(hostname)
            .build()
            .fuse();
        let drain = slog_envlogger::new(drain);
        let drain = slog_async::Async::new(drain).build().fuse();
        slog::Logger::root(drain, slog_o!())
    } else {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_envlogger::new(drain);
        let drain = slog_async::Async::new(drain).build().fuse();
        slog::Logger::root(drain, slog_o!())
    };
    // XXX: cancel slog_scope's NoGlobalLoggerSet for now, it's difficult to
    // prevent it from potentially panicing during tests. reset_logging resets
    // the global logger during shutdown anyway:
    // https://github.com/slog-rs/slog/issues/169
    slog_scope::set_global_logger(logger).cancel_reset();
    slog_stdlog::init().ok();
    Ok(())
}

/// Reset the logger
pub fn reset_logging() {
    let logger = slog::Logger::root(slog::Discard, slog_o!());
    slog_scope::set_global_logger(logger).cancel_reset();
}
