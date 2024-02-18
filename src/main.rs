#![warn(clippy::all)]

use std::error::Error;

use clap::Parser;
use log::{debug, error, info};
use zbus::{blocking::Connection, proxy};

mod geoclue;
mod zoneinfo;

/// Automatically update system timezone based on location
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to zoneinfo tab file
    #[arg(
        short,
        long,
        default_value = "/usr/share/zoneinfo/zone1970.tab",
        env = "AUTOTZD_ZONEINFO_FILE"
    )]
    zoneinfo_path: String,

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

    let mut zoneinfo = zoneinfo::new();
    match zoneinfo.load_zones(&args.zoneinfo_path) {
        Ok(z) => z,
        Err(e) => {
            error!("Failed to load zoneinfo tab file");
            return Err(e);
        }
    };

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

        debug!(
            "Received location update. Latitude: {} / Longitude: {}",
            location.latitude()?,
            location.longitude()?,
        );

        let (timezone, distance) =
            zoneinfo.find_closest_zone(location.latitude()?, location.longitude()?);

        timedate.set_timezone(&timezone, false)?;

        info!(
            "Set timezone to \"{}\" (distance: {:.0}km)",
            timezone,
            distance / 1000.0
        );
    }

    Ok(())
}
