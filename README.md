# Automatic Timezone Daemon

[![Crates.io](https://img.shields.io/crates/v/automatic-timezoned)](https://crates.io/crates/automatic-timezoned)
[![Build Status](https://github.com/maxbrunet/automatic-timezoned/actions/workflows/build.yml/badge.svg)](https://github.com/maxbrunet/automatic-timezoned/actions/workflows/build.yml)
[![dependency status](https://deps.rs/repo/github/maxbrunet/automatic-timezoned/status.svg)](https://deps.rs/repo/github/maxbrunet/automatic-timezoned)

A Linux daemon to automatically update the system timezone based on location.

## How It Works

1. The list of timezones and their location is loaded from the [`zone1970.tab`](https://github.com/eggert/tz/blob/main/zone1970.tab) file
2. The current location is retrieved from GeoClue
3. The distance between the current location and each timezone is calculated with the [Haversine formula](https://en.wikipedia.org/wiki/Haversine_formula)
4. The shortest distance determines the current timezone set via `systemd-timedated`
5. Then, the daemon waits for the location updated signal from GeoClue, and repeats from step 2 when it happens

_Note: The timezone choice may not be accurate if a reference city in a neighboring timezone is closer than any one in the actual timezone._

## Requirements

* [GeoClue](https://gitlab.freedesktop.org/geoclue/geoclue/-/wikis/home)
* [IANA Time Zone Database](https://www.iana.org/time-zones) a.k.a. `tzdata` a.k.a. `zoneinfo`
* [systemd](https://systemd.io/)
* The user must be allowed to use the [`org.freedesktop.timedate1.set-timezone` action](https://www.freedesktop.org/software/systemd/man/org.freedesktop.timedate1.html#Security) (`root` or [Polkit](https://www.freedesktop.org/software/polkit/docs/latest/) rule)
* The user must have a running GeoClue agent or the GeoClue configuration must allow the absence of agent with an empty agent `whitelist`
  (see also [Stebalien/localtime - Configuring GeoClue](https://github.com/Stebalien/localtime#configuring-geoclue), [geoclue/geoclue#74](https://gitlab.freedesktop.org/geoclue/geoclue/-/issues/74))

Please see the [examples/](examples/) directory for sample configurations.

Sample Nix modules can be found here (may be submitted to [NixOS/nixpkgs](https://github.com/NixOS/nixpkgs) if there is interest):

* [maxbrunet/naxos//modules/pkgs/automatic-timezoned.nix](https://github.com/maxbrunet/naxos/blob/main/modules/pkgs/automatic-timezoned.nix)
* [maxbrunet/naxos//modules/services/automatic-timezoned.nix](https://github.com/maxbrunet/naxos/blob/main/modules/services/automatic-timezoned.nix)

## Configuration

```
$ automatic-timezoned --help
Automatically update system timezone based on location

Usage: automatic-timezoned [OPTIONS]

Options:
  -z, --zoneinfo-path <ZONEINFO_PATH>  Path to zoneinfo tab file [env: AUTOTZD_ZONEINFO_FILE=] [default: /usr/share/zoneinfo/zone1970.tab]
  -l, --log-level <LOG_LEVEL>          Log level filter. See <https://docs.rs/env_logger> for syntax [env: AUTOTZD_LOG_LEVEL=] [default: info]
  -h, --help                           Print help information
  -V, --version                        Print version information

```

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

This service depends on the `tzdata` package which allows to update the Time Zone Database independently and does not depend on a third-party service to calculate distances,
here are some alternatives which have made different trade-offs for performance and accuracy:

* [github.com/Stebalien/localtime](https://github.com/Stebalien/localtime): Depends on the unmaintained [github.com/bradfitz/latlong](https://pkg.go.dev/github.com/bradfitz/latlong) Go library.
* [Gnome Automatic Time Zone](https://help.gnome.org/users/gnome-help/stable/clock-timezone.html.en) ([Source Code](https://gitlab.gnome.org/GNOME/gnome-settings-daemon/-/tree/master/plugins/datetime)): Depends on the `tzdata` package and [Nominatim Web API](https://nominatim.org/) for distances.

## License

GNU General Public License v3.0
