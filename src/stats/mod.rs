use std::thread;
use std::sync::mpsc::{Sender};
use std::sync::{Arc, Mutex, mpsc};

#[cfg(feature = "stats-prometheus")]
use prometheus::Counter;

#[cfg(feature = "stats-statsd")]
use std::env;
#[cfg(feature = "stats-statsd")]
use std::net::{Ipv4Addr};
#[cfg(feature = "stats-statsd")]
use std::str::FromStr;
#[cfg(feature = "stats-statsd")]
use cadence::prelude::*;
#[cfg(feature = "stats-statsd")]
use cadence::{StatsdClient, DEFAULT_PORT};

#[cfg(feature = "stats-prometheus")]
lazy_static! {
    static ref HTTP_SUCCESS_COUNTER: Counter = register_counter!(
        opts!(
            "request_http_success",
            "Total number of Successful HTTP requests made.",
            labels!{"service" => "kafka-proxy",
                    "type" => "http",}
        )
    ).unwrap();

    static ref HTTP_FAILURE_COUNTER: Counter = register_counter!(
        opts!(
            "request_http_failure",
            "Total number of Failed HTTP requests made.",
            labels!{"service" => "kafka-proxy",
                    "type" => "http",}
        )
    ).unwrap();

    static ref KAFKA_SUCCESS_COUNTER: Counter = register_counter!(
        opts!(
            "request_kafka_success",
            "Total number of Successful Kafka requests made.",
            labels!{"service" => "kafka-proxy",
                    "type" => "kafka",}
        )
    ).unwrap();

    static ref KAFKA_FAILURE_COUNTER: Counter = register_counter!(
        opts!(
            "request_kafka_failure",
            "Total number of Failed Kafka requests made.",
            labels!{"service" => "kafka-proxy",
                    "type" => "kafka",}
        )
    ).unwrap();
}

#[cfg(feature = "stats-statsd")]
lazy_static! {
    static ref GRAPIHTE_CLIENT: StatsdClient =
        StatsdClient::from_udp_host("kafka.proxy",
            (Ipv4Addr::from_str(&env::var("GRAPHITE_HOST").unwrap()).unwrap(), DEFAULT_PORT)).unwrap();
}

/// A Stat struct to check. contains two fields:
/// `is_http_request` - Whether it was an http request (true), or a kafka request (false).
/// `was_successful` - Whether the http request/kafka request was successful.
#[derive(Debug)]
pub struct Stat {
    pub is_http_request: bool,
    pub was_successful: bool,
}

impl Stat {
    /// A Helper function to create a new "Stat" faster.
    pub fn new(is_http_request: bool, was_successful: bool) -> Stat {
        Stat {
            is_http_request: is_http_request,
            was_successful: was_successful
        }
    }
}

/// A Statistic Reporter.
/// This is a base struct that multiple reporters can implement.
/// Based on features enabled at build time. The following reporters
/// are the ones implemented right now:
/// 1. Prometheus Reporter.
/// 2. StatsD Reporter.
/// 3. NoOp Reporter. (Default)
/// All reporters have merely one function. "start_reporting".
/// Which should return a Arc'd Mutex for a Sender where you send "Stat" instances.
pub struct Reporter;

#[cfg(feature = "stats-prometheus")]
impl Reporter {
    /// Starts the prometheus reporter.
    /// Creates an mpsc channel.
    /// Spawns a thread with an HTTP_SUCCESS_COUNTER, HTTP_FAILURE_COUNTER,
    /// KAFKA_SUCCESS_COUNTER, and KAFKA_FAILURE_COUNTER.
    /// Returns the Sender wrapped in an Arc + Mutex.
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        info!("Starting Prometheus Reporter.");
        thread::spawn(move || {
            loop {
                let possible_stat = rx.try_recv();
                if possible_stat.is_ok() {
                    let stat = possible_stat.unwrap();
                    if stat.is_http_request {
                        if stat.was_successful {
                            HTTP_SUCCESS_COUNTER.inc();
                        } else {
                            HTTP_FAILURE_COUNTER.inc();
                        }
                    } else {
                        if stat.was_successful {
                            KAFKA_SUCCESS_COUNTER.inc();
                        } else {
                            KAFKA_FAILURE_COUNTER.inc();
                        }
                    }
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}

#[cfg(feature = "stats-statsd")]
impl Reporter {
    /// Starts the StatsD reporter.
    /// 1. Reads the GRAPHITE_HOST env var.
    /// 2. Creates an mpsc channel.
    /// 3. Spawns a thread.
    /// 4. Returns the sender wrapped in an Arc + Mutex.
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        info!("Starting StasD Reporter.");
        thread::spawn(move || {
            loop {
                let possible_stat = rx.try_recv();
                if possible_stat.is_ok() {
                    let stat = possible_stat.unwrap();
                    if stat.is_http_request {
                        if stat.was_successful {
                            let _ = GRAPIHTE_CLIENT.incr("http.success");
                        } else {
                            let _ = GRAPIHTE_CLIENT.incr("http.failure");
                        }
                    } else {
                        if stat.was_successful {
                            let _ = GRAPIHTE_CLIENT.incr("kafka.success");
                        } else {
                            let _ = GRAPIHTE_CLIENT.incr("kafka.failure");
                        }
                    }
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}

#[cfg(all(not(feature = "stats-prometheus") , not(feature = "stats-statsd")))]
impl Reporter {
    /// Starts the NoOp reporter.
    /// Creates an mpsc channel.
    /// Spawns a thread.
    /// Returns the mpsc channel wrapped in an arc + mutex.
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        info!("Starting No-OP Reporter.");
        thread::spawn(move || {
            loop {
                let possible_stat = rx.try_recv();
                if possible_stat.is_ok() {
                    let actual_stat = possible_stat.unwrap();
                    info!("Recieved Stat: [ {:?} ].", actual_stat);
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}
