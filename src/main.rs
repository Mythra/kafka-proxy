extern crate bodyparser;
extern crate clap;
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

// Slack Webhook Deps
#[cfg(feature = "reporter-slack")]
extern crate slack_hook;
// END.

mod reporter;
mod stats;

use clap::{Arg, App};
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

fn load_kafka_client(cert_path: path::PathBuf, key_path: path::PathBuf, brokers: Vec<String>) -> KafkaClient {
    let mut context = SslContext::new(SslMethod::Tlsv1).unwrap();
    context.set_cipher_list("DEFAULT").unwrap();
    context.set_certificate_file(&cert_path, X509FileType::PEM).unwrap();
    context.set_private_key_file(&key_path, X509FileType::PEM).unwrap();

    KafkaClient::new_secure(brokers, SecurityConfig::new(context))
}

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
struct MessagePayload {
    topic: String,
    payload: String,
}

fn main() {
    let matches = App::new("kafka-proxy")
        .version("0.7.0")
        .author("Eric <ecoan@instructure.com>")
        .about("Takes in HTTP Posts, and sends their bodies to kafka.")
        .arg(Arg::with_name("brokers")
                .short("b")
                .help("A comma seperated list of kafka brokers to send to. Only Supports IP:PORT")
                .takes_value(true))
        .arg(Arg::with_name("cert_path")
                .short("c")
                .long("certificate")
                .help("The full path to the certificate to use for kafka.")
                .takes_value(true))
        .arg(Arg::with_name("key_path")
                .short("k")
                .long("key_path")
                .help("The full path to the key to use for kafka.")
                .takes_value(true))
        .arg(Arg::with_name("port")
                .short("p")
                .help("The port to listen on for HTTP Posts.")
                .takes_value(true))
        .arg(Arg::with_name("panic_on_backup")
                .short("P")
                .long("panic")
                .help("Whether or not to panic on backup."))
        .arg(Arg::with_name("dry_run")
                .short("d")
                .help("Enabled 'dry run' aka only logging to STDOUT."))
        .get_matches();

    let cert_path: path::PathBuf;
    let key_path: path::PathBuf;
    let brokers: Vec<String>;
    let non_split_brokers: String;
    let port: u64;
    let mut panic_on_backup: bool = false;
    let mut dry_run: bool = false;

    if matches.value_of("cert_path").is_some() {
        cert_path = matches.value_of("cert_path").unwrap().into();
    } else {
        let env_var: String = env::var("KAFKA_PROXY_CERT_PATH").unwrap();
        cert_path = env_var.into();
    }

    if matches.value_of("key_path").is_some() {
        key_path = matches.value_of("key_path").unwrap().into();
    } else {
        let env_var: String = env::var("KAFKA_PROXY_KEY_PATH").unwrap();
        key_path = env_var.into();
    }

    if matches.value_of("brokers").is_some() {
        non_split_brokers = matches.value_of("brokers").unwrap().to_string();
    } else {
        non_split_brokers = env::var("KAFKA_BROKERS").unwrap();
    }

    brokers = non_split_brokers.split(',')
        .map(|raw_hostname| {
            //let hostname_no_prefix = raw_hostname.split("//").skip(1).next().unwrap();
            let mut splitter = raw_hostname.split(":");
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
        .collect();

    if matches.value_of("port").is_some() {
        port = matches.value_of("port").unwrap().parse::<_>().unwrap();
    } else {
        port = env::var("PROXY_PORT").unwrap().parse::<_>().unwrap();
    }

    if matches.occurrences_of("panic_on_backup") > 0 {
        panic_on_backup = true;
    }
    if env::var("PANIC_ON_BACKUP").is_ok() {
        panic_on_backup = true;
    }

    if matches.occurrences_of("dry_run") > 0 {
        dry_run = true;
    }

    let (tx, rx) = mpsc::channel();

    let original_tx = Arc::new(Mutex::new(tx));
    let new_tx = original_tx.clone();

    let db = Store::new("kafka_rust");
    if db.is_err() {
        panic!("Failed to create Backup Store!");
    }
    let db = db.unwrap();

    let kafka_client: KafkaClient;
    let producer;

    if !dry_run {
        kafka_client = load_kafka_client(cert_path, key_path, brokers);
        producer = Some(Producer::from_client(kafka_client).create().unwrap());
    } else {
        producer = None;
    }

    let arcd_producer;
    if !dry_run {
        arcd_producer = Some(Arc::new(Mutex::new(producer.unwrap())));
    } else {
        arcd_producer = None;
    }

    let failed_to_sends = db.get_all::<MessagePayload>();

    if failed_to_sends.is_err() {
        println!("[-] Failed to get all failed to sends. Continuing.")
    }else {
        let failed_to_sends = failed_to_sends.unwrap();
        for (id, message_payload) in failed_to_sends.iter() {
            let cloned = message_payload.clone();

            let attempt_to_send;

            if !dry_run {
                let arcd_producer = arcd_producer.clone().unwrap();
                attempt_to_send = arcd_producer.lock().unwrap().send(&Record {
                    topic: &cloned.topic,
                    partition: -1,
                    key: (),
                    value: cloned.payload,
                });
            } else {
                attempt_to_send = Err(kafka::error::Error::StringDecodeError);
            }

            if attempt_to_send.is_err() {
                if !dry_run {
                    println!("[-] Failed to resend backup message");
                }
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

    println!("[+] Initializing Failure Reporter.");
    let failure_reporter = reporter::Reporter{};
    println!("[+] Starting Failure Reporter.");
    let failed_tx = failure_reporter.start_reporting();
    println!("[+] Done.");

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

                let attempt_to_send;

                if dry_run {
                    println!("{:?}", message_payload);
                    attempt_to_send = Ok(());
                } else {
                    let arcd_producer = arcd_producer.clone().unwrap();
                    attempt_to_send = arcd_producer.lock().unwrap().send(&Record{
                        topic: &message_payload.topic,
                        partition: -1,
                        key: (),
                        value: message_payload.payload,
                    });
                }

                if attempt_to_send.is_err() {
                    let save_result = db.save(&cloned_object);
                    if save_result.is_err() {
                        if panic_on_backup {
                            panic!("[-] Failed to backup: [ {:?} ]", cloned_object);
                        } else {
                            println!("[-] Failed to backup: [ {:?} ]", cloned_object);
                        }
                    } else {
                        println!("[-] Failed to send: [ {:?} ] to kafka, but has been backed up.", cloned_object);
                    }
                    let _ = failed_tx.lock().unwrap().send(());
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

    let url = format!("0.0.0.0:{}", port);

    println!("[+] Starting Kafka Proxy at: [ {:?} ]", url);
    let router = router!(post "/kafka/:topic" => kafka_proxy);
    Iron::new(router).http(&url.as_str()).unwrap();
}
