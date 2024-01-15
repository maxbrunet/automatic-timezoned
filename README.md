# Automatic Timezone Daemon

[![Crates.io](https://img.shields.io/crates/v/automatic-timezoned)](https://crates.io/crates/automatic-timezoned)
[![Build Status](https://github.com/maxbrunet/automatic-timezoned/actions/workflows/build.yml/badge.svg)](https://github.com/maxbrunet/automatic-timezoned/actions/workflows/build.yml)
[![dependency status](https://deps.rs/repo/github/maxbrunet/automatic-timezoned/status.svg)](https://deps.rs/repo/github/maxbrunet/automatic-timezoned)

A Linux daemon to automatically update the system timezone based on location.

## How It Works

1. The current location is retrieved from GeoClue
2. The timezone of the current location is determined using [tzf-rs](https://github.com/ringsaturn/tzf-rs)
3. The timezone is updated via `systemd-timedated`
4. Then, the daemon waits for the location updated signal from GeoClue, and repeats from step 1 when it happens

## Requirements

* [GeoClue](https://gitlab.freedesktop.org/geoclue/geoclue/-/wikis/home)
* [systemd](https://systemd.io/)
* The user must be allowed to use the [`org.freedesktop.timedate1.set-timezone` action](https://www.freedesktop.org/software/systemd/man/org.freedesktop.timedate1.html#Security) (`root` or [Polkit](https://www.freedesktop.org/software/polkit/docs/latest/) rule)
* The user must have a running GeoClue agent or the GeoClue configuration must allow the absence of agent with an empty agent `whitelist`
  (see also [Stebalien/localtime - Configuring GeoClue](https://github.com/Stebalien/localtime#configuring-geoclue), [geoclue/geoclue#74](https://gitlab.freedesktop.org/geoclue/geoclue/-/issues/74))

Please see the [examples/](examples/) directory for sample configurations.

## Configuration

```
$ automatic-timezoned --help
Automatically update system timezone based on location

Usage: automatic-timezoned [OPTIONS]

Options:
  -l, --log-level <LOG_LEVEL>  Log level filter. See <https://docs.rs/env_logger> for syntax [env: AUTOTZD_LOG_LEVEL=] [default: info]
  -h, --help                   Print help
  -V, --version                Print version

```

## Packages

[![Packaging status](https://repology.org/badge/vertical-allrepos/automatic-timezoned.svg?header=&columns=3)](https://repology.org/project/automatic-timezoned/versions)

## Development

### Build

```shell
cargo build --release
```

### Test

```shell
cargo test
```

## Alternatives

This service depends on the `tzf-rs` package which bundles timezones and their geographical borders,
here are some alternatives which have made different trade-offs for performance and accuracy:

* [github.com/Stebalien/localtime](https://github.com/Stebalien/localtime): Depends on the unmaintained [github.com/bradfitz/latlong](https://pkg.go.dev/github.com/bradfitz/latlong) Go library.
* [Gnome Automatic Time Zone](https://help.gnome.org/users/gnome-help/stable/clock-timezone.html.en) ([Source Code](https://gitlab.gnome.org/GNOME/gnome-settings-daemon/-/tree/master/plugins/datetime)): Depends on the `tzdata` package and [Nominatim Web API](https://nominatim.org/) for distances.

## License

GNU General Public License v3.0
