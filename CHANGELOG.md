## 0.8.1 (October 17, 2016)

- Use Rust-Lang log instead of `prinln!`.

## 0.8.0 (October 16, 2016)

- Internal Refactoring.
- Added Tests.
- Make Dry Run not attempt to send stats.

## 0.7.0 (October 15, 2016)

- Allows Env Vars to be passed through CLI Opts.
- Add in dry run CLI Only OPT.
- Fix Bug in Option Parsing.

## 0.6.0 (October 1, 2016)

- Adds in ability to add other failure reporting in the future.
- Build out failure reporting to Slack as a feature (use `--feature reporter-slack`).
- Cleaned up Statistics Reporting Code.

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
