extern crate bodyparser;
extern crate iron;
extern crate jfs;
extern crate kafka;
#[macro_use]
extern crate lazy_static;
extern crate openssl;
#[macro_use]
extern crate router;
extern crate rustc_serialize;

// Prometheus Deps
#[cfg(feature = "stats-prometheus")]
#[macro_use]
extern crate prometheus;
// END.

// StatsD Deps
#[cfg(feature = "stats-statsd")]
extern crate cadence;
// END.

mod stats;

use iron::prelude::*;
use iron::status;
use jfs::Store;
use kafka::client::{SecurityConfig, KafkaClient};
use kafka::producer::{Producer, Record};
use openssl::ssl::{SslContext, SslMethod};
use openssl::x509::X509FileType;
use router::Router;
use std::{env, path, thread};
use std::sync::{Arc, Mutex, mpsc};

lazy_static! {
    static ref KAFKA_BROKERS: Vec<String> = env::var("KAFKA_BROKERS")
        .and_then(|line| {
            Ok(line
                .split(',')
                .map(|raw_hostname| {
                    let hostname_no_prefix = raw_hostname.split("//").skip(1).next().unwrap();
                    let mut splitter = hostname_no_prefix.split(":");
                    let hostname = splitter.next().unwrap();
                    let port = splitter.next().unwrap();
                    /*
                    lookup_host is considered "unstable" as of right now. When it becomes stable
                    uncomment this code so we can do normal dns resolution here, and give domain names
                    not IP Addresses.
                    https://github.com/rust-lang/rust/issues/27705
                    ^--- This is the particular issue regarding `lookup_host`. The reason this is marked
                    as unstable is because of how the C call returns it's info, and how they should
                    represent this. Based on the issue it sounds like they're going to prototypes these methods
                    inside the `net2` crate. However currently net2 only supports tcpstream connecting.
                    if it becomes a problem before it's implemented in std::net it might be implemented
                    in net2 so you should check there.

                    let mut hosts = lookup_host(hostname).unwrap();
                    let mut host = hosts.next().unwrap().unwrap();

                    host.set_port(u16::from_str_radix(port, 10).unwrap());*/
                    format!("{}:{}", hostname, port)
                })
                .collect())
        })
        .unwrap();
    static ref KAFKA_CLIENT_CERT_PATH: path::PathBuf = env::var("KAFKA_PROXY_CERT_PATH")
        .unwrap()
        .into();
    static ref KAFKA_CLIENT_KEY_PATH: path::PathBuf = env::var("KAFKA_PROXY_KEY_PATH")
        .unwrap()
        .into();
    static ref PORT: u64 = env::var("PROXY_PORT")
        .unwrap()
        .parse::<_>()
        .unwrap();
    static ref PANIC_ON_BACKUP: bool = env::var("PANIC_ON_BACKUP").is_ok();
    static ref URL: String = format!("0.0.0.0:{}", *PORT);
}

fn load_kafka_client() -> KafkaClient {
    let mut context = SslContext::new(SslMethod::Tlsv1).unwrap();
    context.set_cipher_list("DEFAULT").unwrap();
    context.set_certificate_file(&*KAFKA_CLIENT_CERT_PATH, X509FileType::PEM).unwrap();
    context.set_private_key_file(&*KAFKA_CLIENT_KEY_PATH, X509FileType::PEM).unwrap();

    KafkaClient::new_secure((*KAFKA_BROKERS).clone(), SecurityConfig::new(context))
}

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
struct MessagePayload {
    topic: String,
    payload: String,
}

fn main() {
    let (tx, rx) = mpsc::channel();

    let original_tx = Arc::new(Mutex::new(tx));
    let new_tx = original_tx.clone();

    let db = Store::new("kafka_rust");
    if db.is_err() {
        panic!("Failed to create Backup Store!");
    }
    let db = db.unwrap();

    let kafka_client: KafkaClient = load_kafka_client();
    let mut producer = Producer::from_client(kafka_client).create().unwrap();

    let failed_to_sends = db.get_all::<MessagePayload>();

    if failed_to_sends.is_err() {
        println!("[-] Failed to get all failed to sends. Continuing.")
    }else {
        let failed_to_sends = failed_to_sends.unwrap();
        for (id, message_payload) in failed_to_sends.iter() {
            let cloned = message_payload.clone();

            let attempt_to_send = producer.send(&Record {
                topic: &cloned.topic,
                partition: -1,
                key: (),
                value: cloned.payload,
            });

            if attempt_to_send.is_err() {
                println!("[-] Failed to resend backup message.");
            } else {
                let _ = db.delete(&id);
            }
        }
    }

    println!("[+] Initializing Metrics Reporter.");
    let reporter = stats::Reporter{};
    println!("[+] Starting Metrics Reporter.");
    let reporter_tx = reporter.start_reporting();
    let http_reporter = reporter_tx.clone();
    let kafka_reporter = reporter_tx.clone();
    println!("[+] Done.");

    let producer = Arc::new(Mutex::new(producer));

    let kafka_proxy = move |ref mut req: &mut Request| -> IronResult<Response> {
        let body = req.get::<bodyparser::Raw>();
        let topic = req.extensions.get::<Router>().unwrap().find("topic").unwrap();
        match body {
            Ok(Some(body)) => {
                &new_tx.lock().unwrap().send(MessagePayload {
                    topic: String::from(topic),
                    payload: body
                }).unwrap();
                let _ = http_reporter.lock().unwrap().send(stats::Stat{
                    is_http_request: true,
                    was_successful: true
                });
                Ok(Response::with(status::Ok))
            },
            Ok(None) => {
                let _ = http_reporter.lock().unwrap().send(stats::Stat{
                    is_http_request: true,
                    was_successful: false
                });
                Ok(Response::with(status::BadRequest))
            },
            Err(_) => {
                let _ = http_reporter.lock().unwrap().send(stats::Stat{
                    is_http_request: true,
                    was_successful: false
                });
                Ok(Response::with(status::BadRequest))
            }
        }
    };

    thread::spawn(move || {
        loop {
            let possible_payload = rx.try_recv();
            if possible_payload.is_ok() {
                let message_payload = possible_payload.unwrap();
                let cloned_object = message_payload.clone();

                let attempt_to_send = producer.lock().unwrap().send(&Record{
                    topic: &message_payload.topic,
                    partition: -1,
                    key: (),
                    value: message_payload.payload,
                });

                if attempt_to_send.is_err() {
                    let save_result = db.save(&cloned_object);
                    if save_result.is_err() {
                        if *PANIC_ON_BACKUP {
                            panic!("[-] Failed to backup: [ {:?} ]", cloned_object);
                        } else {
                            println!("[-] Failed to backup: [ {:?} ]", cloned_object);
                        }
                    } else {
                        println!("[-] Failed to send: [ {:?} ] to kafka, but has been backed up.", cloned_object);
                    }
                    let _ = kafka_reporter.lock().unwrap().send(stats::Stat{
                        is_http_request: false,
                        was_successful: false
                    });
                } else {
                    let _ = kafka_reporter.lock().unwrap().send(stats::Stat{
                        is_http_request: false,
                        was_successful: true
                    });
                }
            }
        }
    });

    println!("[+] Starting Kafka Proxy at: [ {:?} ]", *URL);
    let router = router!(post "/kafka/:topic" => kafka_proxy);
    Iron::new(router).http(&*URL.as_str()).unwrap();
}
