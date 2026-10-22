#![warn(clippy::all)]

use std::process::ExitCode;

use clap::Parser;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
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

fn main() -> ExitCode {
    let args = Args::parse();

    env_logger::Builder::new()
        .parse_filters(&args.log_level)
        .init();

    info!(
        "Starting {} {}...",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    match zbus::block_on(run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

async fn run() -> Result<(), ()> {
    let conn = Connection::system()
        .await
        .map_err(|err| error!("Failed to connect to the system bus: {err}"))?;

    let gclue_manager = geoclue::ManagerProxy::new(&conn)
        .await
        .map_err(|err| error!("Failed to build the GeoClue manager proxy: {err}"))?;
    let gclue_client = gclue_manager
        .get_client()
        .await
        .map_err(|err| error!("Failed to get a GeoClue client: {err}"))?;
    gclue_client
        .set_desktop_id("automatic-timezoned")
        .await
        .map_err(|err| error!("Failed to set the GeoClue client desktop id: {err}"))?;
    gclue_client
        .set_distance_threshold(10000)
        .await
        .map_err(|err| error!("Failed to set the GeoClue client distance threshold: {err}"))?;
    gclue_client
        .set_requested_accuracy_level(geoclue::AccuracyLevel::City as u32)
        .await
        .map_err(|err| error!("Failed to set the GeoClue client accuracy level: {err}"))?;

    let timedate = TimedateProxy::new(&conn)
        .await
        .map_err(|err| error!("Failed to build the timedated proxy: {err}"))?;

    let mut location_updated = gclue_client
        .receive_location_updated()
        .await
        .map_err(|err| error!("Failed to subscribe to GeoClue location updates: {err}"))?;

    gclue_client
        .start()
        .await
        .map_err(|err| error!("Failed to start the GeoClue client: {err}"))?;

    let zone_finder = DefaultFinder::new();

    info!("Waiting for location updates...");
    while let Some(signal) = location_updated.next().await {
        let args = signal
            .args()
            .map_err(|err| error!("Failed to parse the GeoClue location update: {err}"))?;

        let location = geoclue::LocationProxy::builder(&conn)
            .path(args.new())
            .build()
            .await
            .map_err(|err| error!("Failed to build the GeoClue location proxy: {err}"))?;

        let latitude = location
            .latitude()
            .await
            .map_err(|err| error!("Failed to read the location latitude: {err}"))?;
        let longitude = location
            .longitude()
            .await
            .map_err(|err| error!("Failed to read the location longitude: {err}"))?;

        debug!("Received location update. Latitude: {latitude} / Longitude: {longitude}");

        let timezone = zone_finder.get_tz_name(longitude, latitude);

        if timezone.is_empty() {
            warn!("Failed to find a timezone. Latitude: {latitude} / Longitude: {longitude}");
            continue;
        }

        timedate
            .set_timezone(timezone, false)
            .await
            .map_err(|err| error!("Failed to set the timezone: {err}"))?;
        info!("Set timezone to \"{timezone}\"",);
    }

    error!("D-Bus connection closed");
    Err(())
}
