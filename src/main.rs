#![warn(clippy::all)]

use std::error::Error;

use clap::Parser;
use futures_lite::stream::StreamExt;
use log::{debug, error, info};
use tzf_rs::DefaultFinder;
use zbus::{proxy, Connection};

mod geoclue;

/// Automatically update system timezone based on location
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log level filter. See <https://docs.rs/env_logger> for syntax
    #[arg(short, long, default_value = "info", env = "AUTOTZD_LOG_LEVEL")]
    log_level: String,
}

#[proxy(
    default_service = "org.freedesktop.timedate1",
    interface = "org.freedesktop.timedate1",
    default_path = "/org/freedesktop/timedate1"
)]
trait Timedate {
    /// SetTimezone method
    fn set_timezone(&self, timezone: &str, interactive: bool) -> zbus::Result<()>;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    env_logger::Builder::new()
        .parse_filters(&args.log_level)
        .init();

    info!(
        "Starting {} {}...",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let zone_finder = DefaultFinder::new();

    let conn = Connection::system().await?;

    let gclue_manager = geoclue::ManagerProxy::new(&conn).await?;
    let gclue_client = gclue_manager.get_client().await?;
    gclue_client.set_desktop_id("automatic-timezoned").await?;
    gclue_client.set_distance_threshold(10000).await?; // meters
    gclue_client
        .set_requested_accuracy_level(geoclue::AccuracyLevel::City as u32)
        .await?;

    let timedate = TimedateProxy::new(&conn).await?;

    let mut location_updated = gclue_client.receive_location_updated().await?;

    gclue_client.start().await?;

    while let Some(signal) = location_updated.next().await {
        let args = signal.args()?;

        let location = geoclue::LocationProxy::builder(&conn)
            .path(args.new())?
            .build().await?;

        let latitude = location.latitude().await?;
        let longitude = location.longitude().await?;

        debug!("Received location update. Latitude: {latitude} / Longitude: {longitude}");

        let timezone = zone_finder.get_tz_name(longitude, latitude);

        if timezone.is_empty() {
            error!("Failed to find a timezone. Latitude: {latitude} / Longitude: {longitude}");
            continue;
        }

        timedate.set_timezone(timezone, false).await?;
        info!("Set timezone to \"{timezone}\"",);
    }

    Ok(())
}
