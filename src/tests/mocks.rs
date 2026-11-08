use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, MutexGuard};

use futures_util::future::try_join;
use zbus::{
    connection::Builder, interface, message::Header, names::BusName, object_server::SignalEmitter,
    Connection, Guid, ObjectPath,
};

const CLIENT_PATH: &str = "/org/freedesktop/GeoClue2/Client/1";
const OLD_LOCATION_PATH: &str = "/org/freedesktop/GeoClue2/Location/0";
fn check_destination(header: &Header<'_>, expected: &str) -> zbus::fdo::Result<()> {
    let destination = header.destination().map(BusName::as_str);
    if destination == Some(expected) {
        Ok(())
    } else {
        Err(zbus::fdo::Error::Failed(format!(
            "unexpected destination: {destination:?}"
        )))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Update {
    Squamish,
    Oyonnax,
    OutOfRange,
    Kampot,
    Missing,
}

#[derive(Clone, Copy)]
struct Location {
    path: &'static str,
    latitude: f64,
    longitude: f64,
    read_error: Option<&'static str>,
}

const SQUAMISH: Location = Location {
    path: "/org/freedesktop/GeoClue2/Location/1",
    latitude: 49.701_633,
    longitude: -123.155_815,
    read_error: None,
};

const OYONNAX: Location = Location {
    path: "/org/freedesktop/GeoClue2/Location/2",
    latitude: 46.257_580,
    longitude: 5.656_080,
    read_error: None,
};

const OUT_OF_RANGE: Location = Location {
    path: "/org/freedesktop/GeoClue2/Location/empty",
    latitude: 91.0,
    longitude: 0.0,
    read_error: None,
};

const KAMPOT: Location = Location {
    path: "/org/freedesktop/GeoClue2/Location/3",
    latitude: 10.591_015_4,
    longitude: 104.126_958_1,
    read_error: None,
};

const MISSING: Location = Location {
    path: "/org/freedesktop/GeoClue2/Location/missing",
    latitude: 0.0,
    longitude: 0.0,
    read_error: Some("location not found"),
};

impl Update {
    pub(super) const ALL: [Self; 5] = [
        Self::Squamish,
        Self::Oyonnax,
        Self::OutOfRange,
        Self::Kampot,
        Self::Missing,
    ];

    const fn location(self) -> &'static Location {
        match self {
            Self::Squamish => &SQUAMISH,
            Self::Oyonnax => &OYONNAX,
            Self::OutOfRange => &OUT_OF_RANGE,
            Self::Kampot => &KAMPOT,
            Self::Missing => &MISSING,
        }
    }

    pub(super) fn path(self) -> &'static str {
        self.location().path
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    GetClient,
    DesktopId,
    DistanceThreshold,
    AccuracyLevel,
    Start,
    SetTimezone,
    Latitude,
    Longitude,
}

#[derive(Default)]
pub(super) struct Config {
    pub(super) fail: Option<Failure>,
    pub(super) updates: &'static [Update],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    GetClient,
    DesktopId(String),
    DistanceThreshold(u32),
    AccuracyLevel(u32),
    Start,
    LatitudeRead(Update),
    LongitudeRead(Update),
    SetTimezone(SetTimezoneCall),
}

impl Event {
    pub(super) fn failure(&self) -> Failure {
        match self {
            Self::GetClient => Failure::GetClient,
            Self::DesktopId(_) => Failure::DesktopId,
            Self::DistanceThreshold(_) => Failure::DistanceThreshold,
            Self::AccuracyLevel(_) => Failure::AccuracyLevel,
            Self::Start => Failure::Start,
            Self::LatitudeRead(_) => Failure::Latitude,
            Self::LongitudeRead(_) => Failure::Longitude,
            Self::SetTimezone(_) => Failure::SetTimezone,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SetTimezoneCall {
    pub(super) timezone: String,
    pub(super) interactive: bool,
}

#[derive(Clone)]
struct MockState {
    events: Arc<Mutex<Vec<Event>>>,
    fail: Option<Failure>,
}

impl MockState {
    fn new(fail: Option<Failure>) -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            fail,
        }
    }

    fn lock_events(&self) -> MutexGuard<'_, Vec<Event>> {
        self.events.lock().expect("mock event lock poisoned")
    }

    fn record(&self, event: Event) -> zbus::fdo::Result<()> {
        let failure = event.failure();
        self.lock_events().push(event);
        if self.fail == Some(failure) {
            Err(zbus::fdo::Error::Failed(format!("{failure:?} failed")))
        } else {
            Ok(())
        }
    }

    fn events(&self) -> Vec<Event> {
        self.lock_events().clone()
    }
}

pub(super) struct MockBus {
    server: Connection,
    client: Connection,
    state: MockState,
}

impl MockBus {
    pub(super) async fn start(config: Config) -> Self {
        let (server_socket, client_socket) =
            UnixStream::pair().expect("create the p2p socket pair");
        // A fixed GUID keeps the p2p handshake deterministic.
        let guid = Guid::from_static_str("0123456789abcdef0123456789abcdef")
            .expect("parse the fixed p2p GUID");
        let state = MockState::new(config.fail);

        let mut server_builder = Builder::unix_stream(server_socket)
            .server(guid)
            .p2p()
            .serve_at(
                "/org/freedesktop/GeoClue2/Manager",
                MockManager {
                    state: state.clone(),
                },
            )
            .serve_at(
                CLIENT_PATH,
                MockClient {
                    state: state.clone(),
                    updates: config.updates,
                },
            );
        for update in Update::ALL {
            server_builder = server_builder.serve_at(
                update.path(),
                MockLocation {
                    update,
                    state: state.clone(),
                },
            );
        }
        let server_builder = server_builder.serve_at(
            "/org/freedesktop/timedate1",
            MockTimedate {
                state: state.clone(),
            },
        );
        let (server, client) = try_join(
            server_builder.build(),
            Builder::unix_stream(client_socket).p2p().build(),
        )
        .await
        .expect("build the p2p server and client");

        Self {
            server,
            client,
            state,
        }
    }

    pub(super) fn client(&self) -> Connection {
        self.client.clone()
    }

    pub(super) fn events(&self) -> Vec<Event> {
        self.state.events()
    }

    pub(super) async fn emit_update(&self, update: Update) -> zbus::Result<()> {
        let emitter =
            SignalEmitter::new(&self.server, CLIENT_PATH).expect("create the signal emitter");
        let old = ObjectPath::from_static_str(OLD_LOCATION_PATH).expect("valid old location path");
        emitter
            .location_updated(
                old,
                ObjectPath::from_static_str(update.path()).expect("valid location path"),
            )
            .await
    }

    pub(super) async fn shutdown(self) {
        let _ = self.client.close().await;
        let _ = self.server.close().await;
    }
}

struct MockManager {
    state: MockState,
}

#[interface(name = "org.freedesktop.GeoClue2.Manager")]
impl MockManager {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    fn get_client(
        &self,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<ObjectPath<'static>> {
        check_destination(&header, "org.freedesktop.GeoClue2")?;
        self.state.record(Event::GetClient)?;
        Ok(ObjectPath::from_static_str(CLIENT_PATH).expect("valid client path"))
    }
}

struct MockLocation {
    update: Update,
    state: MockState,
}

impl MockLocation {
    fn read(&self, event: Event, coordinate: impl Fn(&Location) -> f64) -> zbus::fdo::Result<f64> {
        let location = self.update.location();
        self.state.record(event)?;
        if let Some(reason) = location.read_error {
            return Err(zbus::fdo::Error::Failed(reason.to_string()));
        }
        Ok(coordinate(location))
    }
}

#[interface(name = "org.freedesktop.GeoClue2.Location")]
impl MockLocation {
    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    #[zbus(property)]
    fn latitude(&self, #[zbus(header)] header: Option<Header<'_>>) -> zbus::fdo::Result<f64> {
        if let Some(header) = &header {
            check_destination(header, "org.freedesktop.GeoClue2")?;
        }
        self.read(Event::LatitudeRead(self.update), |location| {
            location.latitude
        })
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    #[zbus(property)]
    fn longitude(&self, #[zbus(header)] header: Option<Header<'_>>) -> zbus::fdo::Result<f64> {
        if let Some(header) = &header {
            check_destination(header, "org.freedesktop.GeoClue2")?;
        }
        self.read(Event::LongitudeRead(self.update), |location| {
            location.longitude
        })
    }
}

struct MockClient {
    state: MockState,
    updates: &'static [Update],
}

#[interface(name = "org.freedesktop.GeoClue2.Client")]
impl MockClient {
    async fn start(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        check_destination(&header, "org.freedesktop.GeoClue2")?;
        self.state.record(Event::Start)?;

        let old = ObjectPath::from_static_str(OLD_LOCATION_PATH).expect("valid old location path");
        for update in self.updates {
            emitter
                .location_updated(
                    old.clone(),
                    ObjectPath::from_static_str(update.path()).expect("valid location path"),
                )
                .await?;
        }

        Ok(())
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    #[zbus(property)]
    fn set_desktop_id(
        &mut self,
        id: String,
        #[zbus(header)] header: Option<Header<'_>>,
    ) -> zbus::fdo::Result<()> {
        if let Some(header) = &header {
            check_destination(header, "org.freedesktop.GeoClue2")?;
        }
        self.state.record(Event::DesktopId(id))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    #[zbus(property)]
    fn set_distance_threshold(
        &mut self,
        meters: u32,
        #[zbus(header)] header: Option<Header<'_>>,
    ) -> zbus::fdo::Result<()> {
        if let Some(header) = &header {
            check_destination(header, "org.freedesktop.GeoClue2")?;
        }
        self.state.record(Event::DistanceThreshold(meters))
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    #[zbus(property)]
    fn set_requested_accuracy_level(
        &mut self,
        level: u32,
        #[zbus(header)] header: Option<Header<'_>>,
    ) -> zbus::fdo::Result<()> {
        if let Some(header) = &header {
            check_destination(header, "org.freedesktop.GeoClue2")?;
        }
        self.state.record(Event::AccuracyLevel(level))
    }

    #[zbus(signal)]
    async fn location_updated(
        emitter: &SignalEmitter<'_>,
        old: ObjectPath<'_>,
        new: ObjectPath<'_>,
    ) -> zbus::Result<()>;
}

struct MockTimedate {
    state: MockState,
}

#[interface(name = "org.freedesktop.timedate1")]
impl MockTimedate {
    #[expect(
        clippy::fn_params_excessive_bools,
        reason = "the D-Bus SetTimezone signature has an interactive bool"
    )]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "zbus injects the message header by value"
    )]
    fn set_timezone(
        &self,
        timezone: String,
        interactive: bool,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        check_destination(&header, "org.freedesktop.timedate1")?;
        self.state.record(Event::SetTimezone(SetTimezoneCall {
            timezone,
            interactive,
        }))
    }
}
