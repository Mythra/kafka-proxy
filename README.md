# Kafka-Proxy #

Kafka-Proxy is a small webserver that takes in an HTTP Post, and then throws that post off to a Kafka
Topic. The way it's currently setup it requires an SSL Certificate/Key to talk to the Kafka Instances,
because setting up SSL Certs isn't that hard, and you should do it even if you think you don't need
too.

Right now the Kafka Client, and http webserver run on different threads. Although rust is very fast
you shouldn't take the "200 OK" as meaning it has been posted in the kafka topic. Purely that it will
be posted to the kafka topic assuming the server doesn't crash before it gets to it in the queue (
which honestly will only happen if the kafka server is down). This is so we can process http
requests at the rate of thousands per second, and still guarantee that they'll be posted to kafka.

## Installation ##

1. Make sure you have rust installed, and ready to build. [Go Here][rust_link], or use this one-line bash statement:
  `curl -sSf https://static.rust-lang.org/rustup.sh | sh`.

2. To build the dev version simply run: `cargo build`, to build a release run: `cargo build --release`.
  (Note: Kafka-Rust requires you to have LibSnappy in your path: ```To build kafka-rust you'll need libsnappy-dev on your local machine. If that library is not installed in the usual path, you can export the LD_LIBRARY_PATH and LD_RUN_PATH environment variables before issueing cargo build.```).

3. Run the binary located in `target/<debug|release>kafka-proxy` passing in the necessary env vars.

## Env Vars ##

| Name                  | Function                                                                                                                          |
|:----------------------|:----------------------------------------------------------------------------------------------------------------------------------|
| KAFKA_BROKERS         | A comma seperated list of brokers for kafka. Right now this has to be in the form: `ip:port`. Hostname resolution is coming soon. |
| KAFKA_PROXY_CERT_PATH | The path to the certificate file to connect to kafka with.                                                                        |
| KAFKA_PROXY_KEY_PATH  | The path to the key file to connect to kafka with.                                                                                |
| KAFKA_TOPIC           | The topic to post to Kafka With.                                                                                                  |
| PROXY_PORT            | The port for the HTTP Webserver to listen on.                                                                                     |

## Supported Kafka Versions ##

We support the same versions of kafka as `kafka-rust`. Which happens to be:
```
kafka-rust is tested against Kafka 0.8.2.x and regularly used against Kafka 0.9 servers. However, efforts to implement support for new features from the Kafka 0.9 release are just in their beginnings.
```

[rust_link]: https://www.rust-lang.org/en-US/downloads.html
