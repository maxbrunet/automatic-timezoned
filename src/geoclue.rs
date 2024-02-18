use zbus::proxy;
use zvariant::ObjectPath;

#[allow(dead_code)]
pub enum AccuracyLevel {
    None = 0,
    Country = 1,
    City = 4,
    Neighborhood = 5,
    Street = 6,
    Exact = 8,
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Manager",
    default_path = "/org/freedesktop/GeoClue2/Manager"
)]
trait Manager {
    /// GetClient method
    #[zbus(object = "Client")]
    fn get_client(&self);
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Client"
)]
trait Client {
    /// Start method
    fn start(&self) -> zbus::Result<()>;

    /// Stop method
    fn stop(&self) -> zbus::Result<()>;

    /// LocationUpdated signal
    #[zbus(signal)]
    fn location_updated(&self, old: ObjectPath<'_>, new: ObjectPath<'_>) -> zbus::Result<()>;

    /// DesktopId property
    #[zbus(property)]
    fn set_desktop_id(&self, id: &str) -> zbus::Result<()>;

    /// DistanceThreshold property
    #[zbus(property)]
    fn set_distance_threshold(&self, meters: u32) -> zbus::Result<()>;

    /// RequestedAccuracyLevel property
    #[zbus(property)]
    fn set_requested_accuracy_level(&self, level: u32) -> zbus::Result<()>;
}

#[proxy(
    default_service = "org.freedesktop.GeoClue2",
    interface = "org.freedesktop.GeoClue2.Location"
)]
trait Location {
    /// Latitude property
    #[zbus(property)]
    fn latitude(&self) -> zbus::Result<f64>;

    /// Longitude property
    #[zbus(property)]
    fn longitude(&self) -> zbus::Result<f64>;
}
