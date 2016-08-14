extern crate bodyparser;
extern crate iron;
extern crate kafka;
#[macro_use]
extern crate lazy_static;
extern crate openssl;
#[macro_use]
extern crate router;

use iron::prelude::*;
use iron::status;
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
    static ref DEFAULT_TOPIC: String = env::var("DEFAULT_KAFKA_TOPIC")
        .unwrap();
    static ref PORT: u64 = env::var("PROXY_PORT")
        .unwrap()
        .parse::<_>()
        .unwrap();
    static ref URL: String = format!("0.0.0.0:{}", *PORT);
}

fn load_kafka_client() -> KafkaClient {
    let mut context = SslContext::new(SslMethod::Tlsv1).unwrap();
    context.set_cipher_list("DEFAULT").unwrap();
    context.set_certificate_file(&*KAFKA_CLIENT_CERT_PATH, X509FileType::PEM).unwrap();
    context.set_private_key_file(&*KAFKA_CLIENT_KEY_PATH, X509FileType::PEM).unwrap();

    KafkaClient::new_secure((*KAFKA_BROKERS).clone(), SecurityConfig::new(context))
}

struct MessagePayload {
    topic: String,
    payload: String,
}

fn main() {
    let kafka_client: KafkaClient = load_kafka_client();
    let producer = Arc::new(Mutex::new(Producer::from_client(kafka_client)
                            .create().unwrap()));
    let (tx, rx) = mpsc::channel();
    let original_tx = Arc::new(Mutex::new(tx));

    let new_tx = original_tx.clone();

    let kafka_proxy = move |ref mut req: &mut Request| -> IronResult<Response> {
        let body = req.get::<bodyparser::Raw>();
        let topic = req.extensions.get::<Router>().unwrap().find("topic").unwrap_or(&*DEFAULT_TOPIC);
        match body {
            Ok(Some(body)) => {
                &new_tx.lock().unwrap().send(MessagePayload {
                    topic: String::from(topic),
                    payload: body
                }).unwrap();
                Ok(Response::with(status::Ok))
            },
            Ok(None) => {
                Ok(Response::with(status::BadRequest))
            },
            Err(_) => {
                Ok(Response::with(status::BadRequest))
            }
        }
    };

    thread::spawn(move || {
        loop {
            let possible_payload = rx.try_recv();
            if possible_payload.is_ok() {
                let message_payload = possible_payload.unwrap();
                producer.lock().unwrap().send(&Record{
                    topic: &message_payload.topic,
                    partition: -1,
                    key: (),
                    value: message_payload.payload,
                }).unwrap();
            }
        }
    });

    let router = router!(post "/kafka/:topic" => kafka_proxy);
    Iron::new(router).http(&*URL.as_str()).unwrap();
}
