use std::error::Error;

use csv::ReaderBuilder;
use geo::prelude::HaversineDistance;
use geo::{point, Point};
use log::trace;

/// Describes a timezone with its name and location.
#[derive(Debug)]
pub struct Zone {
    pub coordinates: Point,
    pub timezone: String,
}

/// Holds the IANA Time Zone Database
#[derive(Debug)]
pub struct ZoneInfo {
    zones: Vec<Zone>,
}

impl ZoneInfo {
    /// Load zone1970.tab file (https://github.com/eggert/tz/blob/main/zone1970.tab).
    pub fn load_zones(&mut self, path: &str) -> Result<(), Box<dyn Error>> {
        let mut rdr = ReaderBuilder::new()
            .delimiter(b'\t')
            .comment(Some(b'#'))
            .flexible(true)
            .from_path(path)?;

        while let Some(result) = rdr.records().next() {
            let record = result?;
            let coordinates = parse_coordinates(&record[1])?;
            let zone = Zone {
                coordinates,
                timezone: record[2].parse()?,
            };

            trace!(
                "Coordinates \"{}\" parsed to ({}, {}) for \"{}\"",
                &record[1],
                zone.coordinates.y(),
                zone.coordinates.x(),
                zone.timezone
            );

            self.zones.push(zone);
        }

        Ok(())
    }

    /// Calculate the distance between a given location and each zone and return the closest.
    pub fn find_closest_zone(&self, lat: f64, lon: f64) -> (String, f64) {
        let location = point!(x: lon, y: lat);
        let mut timezone = String::from("UTC");
        let mut distance = f64::MAX;

        for z in &self.zones {
            // Haversine over Vincenty/Karney's geodesic,
            // because it is fast and accuracy is not critical
            let d = location.haversine_distance(&z.coordinates);
            if distance > d {
                timezone = z.timezone.clone();
                distance = d;
            }
        }

        (timezone, distance)
    }
}

/// Creates a new zoneinfo.
pub fn new() -> ZoneInfo {
    ZoneInfo { zones: Vec::new() }
}

/// Parse a longitude or latitude ISO-6709 string into `f64`.
fn parse_coordinate_value(valstr: &str, separator: usize) -> Result<f64, Box<dyn Error>> {
    let whole: f64 = valstr[..separator].parse()?;
    let fractionstr: &str = &valstr[separator..];
    let fraction: f64 = fractionstr.parse()?;
    let value: f64 = if whole >= 0.0 {
        whole + fraction / f64::powf(10.0, fractionstr.len() as f64)
    } else {
        whole - fraction / f64::powf(10.0, fractionstr.len() as f64)
    };

    Ok(value)
}

/// Parse coordinates from ISO-6709 string to `geo::Point`.
fn parse_coordinates(coordinates: &str) -> Result<Point, Box<dyn Error>> {
    let mut i = 1;

    for c in coordinates.chars().skip(1) {
        if c == '-' || c == '+' {
            break;
        };
        i += 1;
    }

    let lat = parse_coordinate_value(&coordinates[..i], 3)?; // ±DDMMSS
    let lon = parse_coordinate_value(&coordinates[i..], 4)?; // ±DDDMMSS

    Ok(point!(x: lon, y: lat))
}

#[test]
fn test_parse_coordinate_value() {
    assert_eq!(parse_coordinate_value("+1234", 3).unwrap(), 12.34);
    assert_eq!(parse_coordinate_value("+012345", 3).unwrap(), 1.2345);
    assert_eq!(parse_coordinate_value("-1234567", 4).unwrap(), -123.4567);
}

#[test]
fn test_parse_coordinate() {
    assert_eq!(
        parse_coordinates("+1234+1234").unwrap(),
        point!(x: 123.4, y: 12.34)
    );
    assert_eq!(
        parse_coordinates("+012345-0123456").unwrap(),
        point!(x: -12.3456, y: 1.2345)
    );
    assert_eq!(
        parse_coordinates("-123456+1234567").unwrap(),
        point!(x: 123.4567, y: -12.3456)
    );
}

#[test]
fn test_zoneinfo() {
    let mut zoneinfo = new();
    zoneinfo.load_zones("src/fixtures/zone1970.tab").unwrap();
    insta::assert_debug_snapshot!(zoneinfo);

    // Oyonnax, France
    let (tz1, _) = zoneinfo.find_closest_zone(46.257580, 5.656080);
    // Ideally, it should be "Europe/Paris", but Zurich is closer
    // Anyway still the same timezone in this case
    assert_eq!(tz1, "Europe/Zurich");

    // Squamish, BC, Canada
    let (tz2, _) = zoneinfo.find_closest_zone(49.701633, -123.155815);
    assert_eq!(tz2, "America/Vancouver");
}
