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
use cadence::{StatsdClient, UdpMetricSink, DEFAULT_PORT};

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
    static ref GRAPIHTE_CLIENT: StatsdClient<UdpMetricSink> =
        StatsdClient::<UdpMetricSink>::from_udp_host("kafka.proxy",
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

pub struct Reporter {}

impl Reporter {
    #[cfg(feature = "stats-prometheus")]
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        println!("[+] Starting Prometheus Reporter.");
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

    #[cfg(feature = "stats-statsd")]
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        println!("[+] Starting StasD Reporter.");
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

    #[cfg(all(not(feature = "stats-prometheus") , not(feature = "stats-statsd")))]
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<Stat>>> {
        let (tx, rx) = mpsc::channel::<Stat>();
        println!("[+] Starting No-OP Reporter.");
        thread::spawn(move || {
            loop {
                let possible_stat = rx.try_recv();
                if possible_stat.is_ok() {
                    let actual_stat = possible_stat.unwrap();
                    println!("Recieved Stat: [ {:?} ].", actual_stat);
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}
