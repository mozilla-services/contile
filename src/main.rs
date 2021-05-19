//! Main application entry point

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
";

#[derive(Debug, Deserialize)]
struct Args {
    flag_config: Option<String>,
}

use contile::{logging, server, settings};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let settings = settings::Settings::with_env_and_config_file(&args.flag_config)?;
    init_logging(!settings.human_logs).expect("Logging failed to init");
    debug!("Starting up... {}:{}", &settings.host, &settings.port);
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
        release: sentry::release_name!(),
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
