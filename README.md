# automatic-timezoned

A Linux daemon to automatically update the system timezone based on location.

## How It Works

1. The list of timezones and their location is loaded from the [`zone1970.tab`](https://github.com/eggert/tz/blob/main/zone1970.tab) file
2. The current location is retrieved from Geoclue
3. The distance between the current location and each timezone is calculated with the [Haversine formula](https://en.wikipedia.org/wiki/Haversine_formula)
4. The shortest distance determines the current timezone set via `systemd-timedated`
5. Then, the daemon waits for the location updated signal from Geoclue, and repeats from step 2 when it happens

_Note: The timezone choice may not be accurate if a reference city in a neighboring timezone is closer than any one in the actual timezone._

## Requirements

* [Geoclue](https://gitlab.freedesktop.org/geoclue/geoclue/-/wikis/home)
* [IANA Time Zone Database](https://www.iana.org/time-zones) a.k.a. `tzdata` a.k.a. `zoneinfo`
* [systemd](https://systemd.io/)
* The user must be allowed to use the [`org.freedesktop.timedate1.set-timezone` action](https://www.freedesktop.org/software/systemd/man/org.freedesktop.timedate1.html#Security) (`root` or [Polkit](https://www.freedesktop.org/software/polkit/docs/latest/) rule)

Please see the [examples/](examples/) directory for sample configurations.

## Configuration

Please see:

```shell
automatic-timezoned --help
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
* [Gnome Automatic Time Zone](https://help.gnome.org/users/gnome-help/stable/clock-timezone.html.en) ([Source Code](https://github.com/GNOME/gnome-settings-daemon/tree/master/plugins/datetime)): Depends on the `tzdata` package and [Nominatim Web API](https://nominatim.org/) for distances.

## License

GNU General Public License v3.0
