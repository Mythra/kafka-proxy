## 0.5.0 (September 29, 2016)

- Build out statistics reporting (Defaults to "noop" i.e. just printing to STDOUT).
- Adds in reporting to "prometheus" (use `--feature stats-prometheus`).
- Adds in reporting to "stats" (use `--feature stats-statsd`).

## 0.4.0 (September 28, 2016)

- Don't crash when a message fails to send to kafka.
- Back up messages that fail to send to kafka.
- Attempt to resend messages on restart of the app.

## 0.2.0 (September 15, 2016)

- Initial Release of Kafka Proxy.
