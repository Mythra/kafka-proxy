# Kafka-Proxy #

Kafka-Proxy is a small webserver that takes in an HTTP Post, and then throws that post off to a Kafka
Topic. The way it's currently setup it requires an SSL Certificate/Key to talk to the Kafka Instances,
because setting up SSL Certs isn't that hard, and you should do it even if you think you don't need
too.

Right now the Kafka Client, and http webserver run on different threads. Although rust is very fast
you shouldn't take the "200 OK" as meaning it has been posted in the kafka topic. This is so we can
process http requests at the rate of thousands per second, and still guarantee that they'll be
posted to kafka.

If a message fails to send in kafka it will create a unique file inside of a folder called "kafka_rust".
Kafka Rust will attempt to send messages from this folderthat have failed on reboot. This will
hopefully increase the need for human checking. Even more so for payloads without timestamps in the message.

## Installation ##

1. Make sure you have rust installed, and ready to build. [Go Here][rust_link], or use this one-line bash statement:
  `curl -sSf https://static.rust-lang.org/rustup.sh | sh`.

2. To build the dev version simply run: `cargo build`, to build a release run: `cargo build --release`.
  (Note: Kafka-Rust requires you to have LibSnappy in your path: ```To build kafka-rust you'll need libsnappy-dev on your local machine. If that library is not installed in the usual path, you can export the LD_LIBRARY_PATH and LD_RUN_PATH environment variables before issueing cargo build.```).

3. Run the binary located in `target/<debug|release>/kafka-proxy` passing in the necessary env vars.

### Setting up Stats features ###

Kafka-Proxy allows reporting of HTTP/Kafka stats to some of the most popular solutions out there.
The two of these are prometheus, and statsd (called "Graphite" in some places in the source).
In order to use one of these simply enable: `stats-prometheus`, or `stats-statsd` features at build time,
and setup the env vars.

### Setting up Error Notifying ###

Kafka-Proxy allows alerting when we fail to send to kafka so you can fix the problem manually.
Right now the only supported reporter is over slack, but if you want it to report somewhere else feel
free to open an issue/PR implementing it.

In order to use slack simply enable the feature: `reporter-slack` at build time, and setup the env vars.

## Env Vars ##

It should be noted env vars can be passed through CLI Opts in v0.7.0

| Name                  | Optional  | Function                                                                                                                          |
|:----------------------|:----------|:----------------------------------------------------------------------------------------------------------------------------------|
| GRAPHITE_HOST         | Sometimes | The IPv4 Address of the Graphite Host to POST results to for reporting with statsd.                                               |
| KAFKA_BROKERS         | No        | A comma seperated list of brokers for kafka. Right now this has to be in the form: `ip:port`. Hostname resolution is coming soon. |
| KAFKA_PROXY_CERT_PATH | No        | The path to the certificate file to connect to kafka with.                                                                        |
| KAFKA_PROXY_KEY_PATH  | No        | The path to the key file to connect to kafka with.                                                                                |
| PANIC_ON_BACKUP       | Yes       | Whether the program should crash if we fail to backup a message that failed to send to kafka.                                     |
| PROXY_PORT            | No        | The port for the HTTP Webserver to listen on.                                                                                     |
| SLACK_WEBHOOK         | Sometimes | The Slack Webhook URL to connect to slack.                                                                                        |
| SLACK_CHANNEL         | Yes       | The slack channel to post to. Defaults to "#general".                                                                             |
| NO_SSL                | Yes       | Whether to blacklist ssl.                                                                                                         |

Finally logging is setup through the rust crate `log`, and `env_logger`. As such
the logging level printed to STDOUT/STDERR is determined by the env var: `RUST_LOG`.
Which I recommend setting at `INFO`, unless you need `DEBUG` for the advanced logs.

## Supported Kafka Versions ##

We support the same versions of kafka as `kafka-rust`. Which happens to be:
```
kafka-rust is tested against Kafka 0.8.2.x and regularly used
against Kafka 0.9 servers. However, efforts to implement support
for new features from the Kafka 0.9 release are just in their
beginnings.
```

[rust_link]: https://www.rust-lang.org/en-US/downloads.html
