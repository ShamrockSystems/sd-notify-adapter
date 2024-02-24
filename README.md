# Health Status Endpoint Adapter for `systemd` `NOTIFY_SOCKET` Services

This project serves as an adapter for services that send status information through the `systemd` `NOTIFY_SOCKET` Unix domain socket. It is designed to be run as a sidecar inside of a Kubernetes `Pod` resource.

It serves the following endpoints:

- `/healthz`: Whether the adapter is ready to serve requests. This can be used as the startup probe for the sidecar container.
  - `503`: The _adapter_ is **not** ready to receive messages
  - `200`: The _adapter_ is ready to receive messages
- `/livez`: Whether the service is live. This can be used as the liveness probe for the `Pod`.
  - `503`: The service is **not** live
  - `200`: The service is live
- `/readyz`: Whether the service is ready. This can be used as the readiness probe for the `Pod`.
  - `503`: The service is **not** ready
  - `200`: The service is ready

Additionally, each endpoint returns a JSON response in the following format:

```json5
{
  timestamp: "1970-01-01T00:00:00+00:00", // An RFC 3339 timestamp
  healthz: true, // The value of the `/healthz` endpoint
  livez: true, // The value of the `/healthz` endpoint
  readyz: true, // The value of the `/readyz` endpoint
}
```

## Environment variable configuration

### General configuration

- `NOTIFY_SOCKET`

  _default `/var/run/adapter/adapter.sock`_

  The path of the socket to create

- `ADAPTER_PORT`

  _default `8089`_

  The port for the HTTP server to listen to

- `ADAPTER_ECHO`

  _default `true`_

  - If `true`, the adapter will reserialze all messages to standard output
  - If `false`, there is no standard output

- `ADAPTER_LOG`

  _default `true`_

  - If `true`, the adapter will log JSONL records to standard error
  - If `false`, there is no standard error

- `ADAPTER_CHANNEL_SIZE`

  _default `32`_

  The channel size to use for internal message-passing

- `ADAPTER_INITIAL_LIVEZ`

  _default `false`_

  - If `true`, `/livez` will return that the service is live until a status change occurs
  - If `false`, `/livez` will return that the service is **not** live until a status change occurs

- `ADAPTER_INITIAL_READYZ`

  _default `false`_

  - If `true`, `/readyz` will return that the service is ready until a status change occurs
  - If `false`, `/readyz` will return that the service is **not** ready until a status change occurs

- `ADAPTER_ALLOW_MESSAGE_WATCHDOG_USEC`

  _default `true`_

  - If `true`, the adapter will process `WATCHDOG_USEC` messages and override the current configuration of `ADAPTER_UNIT_WATCHDOG_SEC` (in microseconds).
  - If `false`, the adapter ignores `WATCHDOG_USEC` messages.

- `ADAPTER_ALLOW_MESSAGE_EXTEND_TIMEOUT_USEC`

  _default `true`_

  - If `true`, the adapter will process `EXTEND_TIMEOUT_USEC` messages and extend the startup timeout configured by `ADAPTER_UNIT_TIMEOUT_START_SEC` by the specified number of microseconds.
  - If `false`, the adapter ignores `EXTEND_TIMEOUT_USEC` messages.

### Status change configuration

How `/livez` and `/readyz` react to adapter events is configurable. If multiple messages are processed at the same time, `false` reactions have priority over `true` reactions.

- `ADAPTER_STATUS_LIVEZ_TRUE`

  _default `ready,watchdog`_

  Comma-separated list of events to react to, changing the status of `/livez` to `true`

- `ADAPTER_STATUS_LIVEZ_FALSE`

  _default `errno,buserror,watchdog_trigger,watchdog_timeout,start_timeout`_

  Comma-separated list of events to react to, changing the status of `/livez` to `false`

- `ADAPTER_STATUS_READYZ_TRUE`

  _default `ready,watchdog`_

  Comma-separated list of events to react to, changing the status of `/readyz` to `true`

- `ADAPTER_STATUS_READYZ_FALSE`

  _default `reloading,stopping,errno,buserror,watchdog_trigger,watchdog_timeout,start_timeout`_

  Comma-separated list of events to react to, changing the status of `/readyz` to `false`

- `ADAPTER_STATUS_SHUTDOWN`

  _default empty_

  Comma-separated list of events to react to, shutting down the adapter

#### Adapter events

- `ready`: the adapter has processed a `READY=1` message
- `reloading`: the adapter has processed a `RELOADING=1` message
- `stopping`: the adapter has processed a `STOPPING=1` message
- `errno`: the adapter has proccesed a `ERRNO=...` message
- `buserror`: the adapter has proccesed a `BUSERROR=...` message
- `watchdog`: the adapter has processed a `WATCHDOG=1` message
- `watchdog_trigger`: the adapter has processed a `WATCHDOG=trigger` message
- `watchdog_timeout`: the watchdog has timed out waiting for `WATCHDOG=1`
- `start_timeout`: the startup timer has timed out waiting for `READY=1`

### `systemd` unit configuration

Refer to the [`systemd.service` man page](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html#) for additional details.

- `ADAPTER_UNIT_TIMEOUT_START_SEC`

  _default `90`_

  Roughly equivalent to `TimeoutStartSec=` in a `systemd` unit configuration; sends the `start_timeout` event if the first `READY=1` message is not sent in time.

- `ADAPTER_UNIT_WATCHDOG_SEC`

  _default `0` (disabled)_

  Roughly equivalent to `WatchdogSec=` in a `systemd` unit configuration; enables watchdog functionality. By default it is set to `0` (disabled).

## Supported messages

The adapter can process the following messages. If an unsupported but well-known message is received, it may be parsed and echoed, but is otherwise ignored. If an unknown message is received, it is only mentioned in the error log. Refer to the [`sd_notify` man page](https://www.freedesktop.org/software/systemd/man/latest/sd_notify.html#Well-known%20assignments) for additional details.

| Message                   | Purpose                                                            |
| ------------------------- | ------------------------------------------------------------------ |
| `READY=1`                 | Sends event `ready`                                                |
| `RELOADING=1`             | Sends event `reloading`                                            |
| `STOPPING=1`              | Sends event `stopping`                                             |
| `ERRNO=...`               | Sends event `errno`                                                |
| `BUSERROR=...`            | Sends event `buserror`                                             |
| `WATCHDOG=1`              | Sends event `watchdog` and updates the watchdog                    |
| `WATCHDOG=trigger`        | Sends event `watchdog_trigger`                                     |
| `WATCHDOG_USEC=...`       | Supported if `ADAPTER_ALLOW_MESSAGE_WATCHDOG_USEC` is `true`       |
| `EXTEND_TIMEOUT_USEC=...` | Supported if `ADAPTER_ALLOW_MESSAGE_EXTEND_TIMEOUT_USEC` is `true` |

## Development

### Setup

1. `pre-commit install`
2. `rustup install nightly`
