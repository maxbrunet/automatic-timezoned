#![warn(clippy::all)]

use std::error::Error;

use clap::Parser;
use log::{debug, error, info};
use tzf_rs::DefaultFinder;
use zbus::{blocking::Connection, proxy};

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

fn main() -> Result<(), Box<dyn Error>> {
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

    let conn = Connection::system()?;

    let gclue_manager = geoclue::ManagerProxyBlocking::new(&conn)?;
    let gclue_client = gclue_manager.get_client()?;
    gclue_client.set_desktop_id("automatic-timezoned")?;
    gclue_client.set_distance_threshold(10000)?; // meters
    gclue_client.set_requested_accuracy_level(geoclue::AccuracyLevel::City as u32)?;

    let timedate = TimedateProxyBlocking::new(&conn)?;

    let location_updated = gclue_client.receive_location_updated()?;

    gclue_client.start()?;

    for signal in location_updated {
        let args = signal.args()?;

        let location = geoclue::LocationProxyBlocking::builder(&conn)
            .path(args.new())?
            .build()?;

        let latitude = location.latitude()?;
        let longitude = location.longitude()?;

        debug!(
            "Received location update. Latitude: {} / Longitude: {}",
            latitude, longitude,
        );

        let timezone = zone_finder.get_tz_name(longitude, latitude);

        if timezone.is_empty() {
            error!(
                "Failed to find a timezone. Latitude: {} / Longitude: {}",
                latitude, longitude,
            );
            continue;
        }

        timedate.set_timezone(timezone, false)?;
        info!("Set timezone to \"{}\"", timezone,);
    }

    Ok(())
}
