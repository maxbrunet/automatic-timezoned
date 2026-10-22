# Failure

## System bus unreachable

```console
$ AUTOTZD_LOG_LEVEL=info DBUS_SYSTEM_BUS_ADDRESS=unix:path=/nonexistent automatic-timezoned
? 1
[INFO  automatic_timezoned] Starting automatic-timezoned [VERSION]...
[ERROR automatic_timezoned] Failed to connect to the system bus: failed to connect to [..]

```
