//! Main application entry point
#![forbid(unsafe_code)]
use std::borrow::Cow;
use std::error::Error;

#[macro_use]
extern crate slog_scope;

use docopt::Docopt;
use logging::init_logging;
use serde::Deserialize;

const USAGE: &str = "
Usage: contile [options]

Options:
    -h, --help               Show this message.
    --config=CONFIGFILE      Configuration file path.
    --debug-settings         Turn on logging to Debug settings
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_config: Option<String>,
    flag_debug_settings: Option<bool>,
}

use contile::{create_app_version, logging, server, settings};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    // Optionally turn on logging easier to display any errors around
    // logging intialization.
    if Some(true) == args.flag_debug_settings {
        init_logging(true).expect("could not initilalize logging");
    }
    let settings = settings::Settings::with_env_and_config_file(&args.flag_config, false)?;
    init_logging(!settings.human_logs).expect("Logging failed to init");
    debug!("Intitializing... {}:{}", &settings.host, &settings.port);
    // Set SENTRY_DSN env var to enable Sentry.actix_cors
    // Avoid its default reqwest transport for now due to issues w/
    // likely grpcio's boringssl
    /*
    let curl_transport_factory = |options: &sentry::ClientOptions| {
        Arc::new(sentry::transports::CurlHttpTransport::new(&options))
            as Arc<dyn sentry::internals::Transport>
    };
    */
    let _sentry = sentry::init(sentry::ClientOptions {
        // Note: set "debug: true," to diagnose sentry issues
        // transport: Some(Arc::new(curl_transport_factory)),

        // Use "@" as the separator to be consistent with the default `release`
        // string generated by Sentry.
        release: Some(Cow::Owned(create_app_version("@"))),
        ..sentry::ClientOptions::default()
    });

    debug!("Starting up...");

    let banner = settings.banner();
    let server = server::Server::with_settings(settings).await.unwrap();
    info!("Server running on {}", banner);
    server.await?;
    info!("Server closing");
    logging::reset_logging();

    Ok(())
}
