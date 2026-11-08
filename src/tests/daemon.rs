use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Once;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::mocks::{Config, Event, Failure, MockBus, SetTimezoneCall, Update};
use crate::run;

const VANCOUVER_TIMEZONE: &str = "America/Vancouver";
const PARIS_TIMEZONE: &str = "Europe/Paris";
const KAMPOT_TIMEZONE: &str = "Asia/Phnom_Penh";
const CITY_ACCURACY_LEVEL: u32 = 4;
const RUN_TIMEOUT: Duration = Duration::from_secs(10);

thread_local! {
    static CAPTURE: RefCell<Option<mpsc::Sender<(log::Level, String)>>> =
        const { RefCell::new(None) };
}

struct TestLogger;

impl log::Log for TestLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        CAPTURE.with(|capture| {
            if let Some(tx) = capture.borrow().as_ref() {
                let _ = tx.send((record.level(), record.args().to_string()));
            }
        });
    }

    fn flush(&self) {}
}

static LOGGER: TestLogger = TestLogger;
static LOG_INIT: Once = Once::new();

fn install_test_logger() {
    LOG_INIT.call_once(|| {
        log::set_logger(&LOGGER).expect("set the test logger");
        log::set_max_level(log::LevelFilter::Info);
    });
}

fn timezone_call(timezone: &str) -> SetTimezoneCall {
    SetTimezoneCall {
        timezone: timezone.to_string(),
        interactive: false,
    }
}

fn desktop_id() -> Event {
    Event::DesktopId("automatic-timezoned".to_string())
}

fn setup_events() -> [Event; 4] {
    [
        Event::GetClient,
        desktop_id(),
        Event::DistanceThreshold(10000),
        Event::AccuracyLevel(CITY_ACCURACY_LEVEL),
    ]
}

fn assert_client_configured(events: &[Event]) -> &[Event] {
    let start = events
        .iter()
        .position(|event| event == &Event::Start)
        .unwrap_or_else(|| panic!("Start should be called; events: {events:?}"));
    let setup = &events[..start];
    let expected = setup_events();
    assert_eq!(
        setup.len(),
        expected.len(),
        "unexpected client setup event count: {setup:?}"
    );
    for event in &expected {
        assert!(
            setup.contains(event),
            "missing client setup event {event:?}: {setup:?}"
        );
    }
    let tail = &events[start + 1..];
    assert!(
        !tail.contains(&Event::Start),
        "Start should be called once: {tail:?}"
    );
    assert!(
        !tail.iter().any(|event| matches!(
            event,
            Event::GetClient
                | Event::DesktopId(_)
                | Event::DistanceThreshold(_)
                | Event::AccuracyLevel(_)
        )),
        "client setup events after Start: {tail:?}"
    );
    tail
}

fn set_timezones(events: &[Event]) -> Vec<&SetTimezoneCall> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::SetTimezone(call) => Some(call),
            _ => None,
        })
        .collect()
}

fn wait_for_log(rx: &Receiver<(log::Level, String)>, level: log::Level, needle: &str) {
    let deadline = Instant::now() + RUN_TIMEOUT;
    loop {
        let (record_level, line) = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|err| {
                panic!("expected {level:?} log line containing {needle:?}: {err}")
            });
        if record_level == level && line.contains(needle) {
            break;
        }
    }
}

struct TestRun {
    bus: Option<MockBus>,
    handle: Option<JoinHandle<()>>,
    rx: Receiver<Result<(), ()>>,
}

impl TestRun {
    fn start_with_log(config: Config, log_tx: Option<mpsc::Sender<(log::Level, String)>>) -> Self {
        let bus = zbus::block_on(MockBus::start(config));
        let client = bus.client();
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            CAPTURE.with(|capture| *capture.borrow_mut() = log_tx);
            let result = zbus::block_on(run(&client));
            CAPTURE.with(|capture| *capture.borrow_mut() = None);
            let _ = tx.send(result);
        });
        Self {
            bus: Some(bus),
            handle: Some(handle),
            rx,
        }
    }

    fn close_client(&self) {
        if let Some(bus) = &self.bus {
            let _ = zbus::block_on(bus.client().close());
        }
    }

    fn finish(mut self, timeout: Duration) -> (Result<(), ()>, Vec<Event>) {
        let result = match self.rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => {
                let events = self.events();
                self.shutdown();
                panic!("run did not finish within {timeout:?}; events: {events:?}");
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.shutdown();
                self.resume_run_panic();
            }
        };
        self.join_run();
        let events = self.events();
        (result, events)
    }

    fn bus(&self) -> &MockBus {
        self.bus.as_ref().expect("bus already shut down")
    }

    fn events(&self) -> Vec<Event> {
        self.bus().events()
    }

    fn emit_update(&self, update: Update) {
        zbus::block_on(self.bus().emit_update(update)).expect("emit the location update");
    }

    fn join_run(&mut self) {
        if let Some(handle) = self.handle.take() {
            if let Err(payload) = handle.join() {
                std::panic::resume_unwind(payload);
            }
        }
    }

    fn resume_run_panic(&mut self) -> ! {
        let payload = self
            .handle
            .take()
            .expect("run thread handle")
            .join()
            .expect_err("run task ended without sending a result");
        std::panic::resume_unwind(payload);
    }

    fn shutdown(&mut self) {
        if let Some(bus) = self.bus.take() {
            zbus::block_on(bus.shutdown());
        }
    }
}

#[test]
fn run_sets_timezone_and_skips_unknown_locations() {
    install_test_logger();
    let (log_tx, log_rx) = mpsc::channel();
    let run = TestRun::start_with_log(
        Config {
            updates: &Update::ALL,
            ..Config::default()
        },
        Some(log_tx),
    );
    let (result, events) = run.finish(RUN_TIMEOUT);

    assert!(result.is_err(), "run should fail on the missing location");
    let tail = assert_client_configured(&events);
    assert_eq!(
        set_timezones(tail),
        vec![
            &timezone_call(VANCOUVER_TIMEZONE),
            &timezone_call(PARIS_TIMEZONE),
            &timezone_call(KAMPOT_TIMEZONE),
        ],
        "unexpected timezone calls"
    );
    assert!(
        log_rx
            .try_iter()
            .any(|(level, line)| level == log::Level::Error
                && line.contains("Failed to read the location")),
        "run should log the missing location read failure"
    );
}

#[test]
fn run_fails_when_the_connection_closes() {
    install_test_logger();
    let (log_tx, log_rx) = mpsc::channel();
    let run = TestRun::start_with_log(
        Config {
            updates: &[Update::Squamish, Update::OutOfRange],
            ..Config::default()
        },
        Some(log_tx),
    );

    wait_for_log(&log_rx, log::Level::Warn, "Failed to find a timezone");
    run.emit_update(Update::Kampot);
    wait_for_log(
        &log_rx,
        log::Level::Info,
        "Set timezone to \"Asia/Phnom_Penh\"",
    );
    run.close_client();
    let (result, events) = run.finish(RUN_TIMEOUT);
    assert!(
        result.is_err(),
        "run should fail when the connection closes"
    );
    assert_eq!(
        set_timezones(&events),
        vec![
            &timezone_call(VANCOUVER_TIMEZONE),
            &timezone_call(KAMPOT_TIMEZONE),
        ],
        "unexpected timezone calls"
    );
    assert!(
        log_rx
            .try_iter()
            .any(|(level, line)| level == log::Level::Error
                && line.contains("D-Bus connection closed")),
        "run should log the closed connection"
    );
}

#[test]
fn run_propagates_set_timezone_errors() {
    install_test_logger();
    let (log_tx, log_rx) = mpsc::channel();
    let run = TestRun::start_with_log(
        Config {
            fail: Some(Failure::SetTimezone),
            updates: &[Update::Squamish],
        },
        Some(log_tx),
    );

    let (result, events) = run.finish(RUN_TIMEOUT);
    assert!(result.is_err(), "run should fail");
    let tail = assert_client_configured(&events);
    assert_eq!(
        set_timezones(tail),
        vec![&timezone_call(VANCOUVER_TIMEZONE)],
        "unexpected timezone calls"
    );
    let errors: Vec<_> = log_rx
        .try_iter()
        .filter(|(level, line)| {
            *level == log::Level::Error && line.contains("Failed to set the timezone")
        })
        .collect();
    assert_eq!(
        errors.len(),
        1,
        "run should log the timezone failure once: {errors:?}"
    );
}

#[test]
fn run_fails_during_setup() {
    install_test_logger();
    let cases = [
        (
            Failure::GetClient,
            Event::GetClient,
            "Failed to get a GeoClue client",
        ),
        (
            Failure::DesktopId,
            desktop_id(),
            "Failed to set the GeoClue client desktop id",
        ),
        (
            Failure::DistanceThreshold,
            Event::DistanceThreshold(10000),
            "Failed to set the GeoClue client distance threshold",
        ),
        (
            Failure::AccuracyLevel,
            Event::AccuracyLevel(CITY_ACCURACY_LEVEL),
            "Failed to set the GeoClue client accuracy level",
        ),
        (
            Failure::Start,
            Event::Start,
            "Failed to start the GeoClue client",
        ),
    ];

    for (failure, expected, message) in cases {
        let (log_tx, log_rx) = mpsc::channel();
        let run = TestRun::start_with_log(
            Config {
                fail: Some(failure),
                ..Config::default()
            },
            Some(log_tx),
        );
        let (result, events) = run.finish(RUN_TIMEOUT);
        assert!(result.is_err(), "run should fail for {failure:?}");
        assert_eq!(
            events.last(),
            Some(&expected),
            "unexpected events for {expected:?}: {events:?}"
        );
        assert_eq!(
            events.iter().filter(|event| *event == &expected).count(),
            1,
            "the failing event should be recorded once: {events:?}"
        );
        let errors: Vec<_> = log_rx
            .try_iter()
            .filter(|(level, line)| *level == log::Level::Error && line.contains(message))
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "run should log {message:?} once for {failure:?}: {errors:?}"
        );
    }
}

#[test]
fn geoclue_config_enables_the_package() {
    let config = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/geoclue.conf"
    ));
    let section = format!("[{}]", env!("CARGO_PKG_NAME"));
    let body: Vec<&str> = config
        .lines()
        .skip_while(|line| line.trim() != section)
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .map(str::trim)
        .collect();
    assert!(
        !body.is_empty(),
        "examples/geoclue.conf must contain {section}"
    );
    assert!(
        body.contains(&"allowed=true"),
        "the {section} section must set allowed=true"
    );
    assert!(
        body.contains(&"system=true"),
        "the {section} section must set system=true"
    );
    assert!(
        body.iter().any(|line| line.starts_with("users=")),
        "the {section} section must set users="
    );
}

#[test]
fn run_fails_when_a_coordinate_read_fails() {
    install_test_logger();
    let cases = [
        (
            Failure::Latitude,
            Event::LatitudeRead(Update::Squamish),
            "latitude",
        ),
        (
            Failure::Longitude,
            Event::LongitudeRead(Update::Squamish),
            "longitude",
        ),
    ];

    for (failure, expected, coordinate) in cases {
        let (log_tx, log_rx) = mpsc::channel();
        let run = TestRun::start_with_log(
            Config {
                fail: Some(failure),
                updates: &[Update::Squamish],
            },
            Some(log_tx),
        );

        let (result, events) = run.finish(RUN_TIMEOUT);
        assert!(result.is_err(), "run should fail for {failure:?}");
        let tail = assert_client_configured(&events);
        assert!(
            set_timezones(tail).is_empty(),
            "no timezone should be set for {failure:?}"
        );
        assert!(
            tail.contains(&expected),
            "missing {expected:?} for {failure:?}: {events:?}"
        );
        let errors: Vec<_> = log_rx
            .try_iter()
            .filter(|(level, line)| {
                *level == log::Level::Error
                    && line.contains(&format!("Failed to read the location {coordinate}"))
            })
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "run should log the {coordinate} read failure once: {errors:?}"
        );
    }
}
