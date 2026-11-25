mod cli;
mod fmt;
mod parse;

use std::{path::PathBuf, process::ExitCode};

use anyhow::anyhow;
use clap::ArgMatches;
use dirs::config_dir;
use openscq30_lib::OpenSCQ30Session;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, filter::filter_fn, prelude::*};

#[tokio::main]
async fn main() -> ExitCode {
    let matches = cli::build().get_matches();

    if let Err(err) = initialize_logging(&matches) {
        eprintln!("Logging error: {err:?}");
        return ExitCode::FAILURE;
    }

    if let Err(err) = cli::handle(&matches).await {
        if matches.get_count("verbose") > 0 || matches.get_flag("debug-errors") {
            eprintln!("Error: {err:?}");
        } else {
            // display anyhow context chain on one line
            eprintln!("Error: {err:#}");
        }
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

pub async fn openscq30_session() -> anyhow::Result<OpenSCQ30Session> {
    let db_path = match std::env::var_os("OPENSCQ30_DATABASE_PATH") {
        Some(path) => PathBuf::from(path),
        None => config_dir()
            .ok_or_else(|| anyhow!("failed to find config dir"))?
            .join("openscq30")
            .join("database.sqlite"),
    };
    OpenSCQ30Session::new(db_path).await.map_err(Into::into)
}

fn initialize_logging(matches: &ArgMatches) -> anyhow::Result<()> {
    let log_level_filter = match matches.get_count("verbose") {
        0 => None,
        1 => Some(LevelFilter::WARN),
        2 => Some(LevelFilter::INFO),
        3 => Some(LevelFilter::DEBUG),
        _ => Some(LevelFilter::TRACE),
    };

    if let Some(log_level_filter) = log_level_filter {
        let pilot = matches.subcommand_name() == Some("pilot");
        let output = tracing_subscriber::fmt::layer()
            .with_file(true)
            .with_line_number(true)
            .with_target(true)
            .with_writer(std::io::stderr)
            .pretty()
            // Native Bluetooth libraries may trace device addresses and D-Bus paths.
            .with_filter(filter_fn(move |metadata| {
                !pilot || metadata.target().starts_with("openscl4_")
            }));
        tracing_subscriber::registry()
            .with(
                EnvFilter::builder()
                    .with_default_directive(log_level_filter.into())
                    .from_env()?,
            )
            .with(output)
            .init();
    }
    Ok(())
}
