use zbus::{proxy, ObjectPath};

#[expect(
    dead_code,
    reason = "the full GeoClue accuracy scale is documented; only City is used"
)]
pub(crate) enum AccuracyLevel {
    None,
    Country,
    City,
    Neighborhood,
    Street,
    Exact,
}

impl From<AccuracyLevel> for u32 {
    fn from(level: AccuracyLevel) -> Self {
        match level {
            AccuracyLevel::None => 0,
            AccuracyLevel::Country => 1,
            AccuracyLevel::City => 4,
            AccuracyLevel::Neighborhood => 5,
            AccuracyLevel::Street => 6,
            AccuracyLevel::Exact => 8,
        }
    }
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Manager",
    default_path = "/org/freedesktop/GeoClue2/Manager"
)]
pub trait Manager {
    #[zbus(object = "Client")]
    fn get_client(&self);
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Client"
)]
pub trait Client {
    fn start(&self) -> zbus::Result<()>;

    fn stop(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn location_updated(&self, old: ObjectPath<'_>, new: ObjectPath<'_>) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_desktop_id(&self, id: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_distance_threshold(&self, meters: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_requested_accuracy_level(&self, level: u32) -> zbus::Result<()>;
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Location"
)]
pub trait Location {
    #[zbus(property)]
    fn latitude(&self) -> zbus::Result<f64>;

    #[zbus(property)]
    fn longitude(&self) -> zbus::Result<f64>;
}
