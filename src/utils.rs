use clap::{App, Arg, ArgMatches};
use jfs::Store;
use kafka::producer::{Producer, Record};
use ::models::{Configuration, MessagePayload};
use std::{env, path};
use std::sync::{Arc, Mutex};

/// Initializze the Clap Application.
/// Basically a sole entry point to the CLI Option Parsing.
/// So that way we don't have to define it again for testing.
pub fn initialize_app() -> App<'static, 'static> {
    App::new("kafka-proxy").version("0.8.0").author("Eric <ecoan@instructure.com>")
        .about("Takes in HTTP Posts, and sends their bodies to kafka.")
        .arg(Arg::with_name("brokers").short("b")
                .help("A comma seperated list of kafka brokers to send to. Only Supports IP:PORT").takes_value(true))
        .arg(Arg::with_name("cert_path").short("c").long("certificate")
            .help("The full path to the certificate to use for kafka.").takes_value(true))
        .arg(Arg::with_name("key_path").short("k").long("keypath")
                .help("The full path to the key to use for kafka.").takes_value(true))
        .arg(Arg::with_name("port").short("p")
                .help("The port to listen on for HTTP Posts.").takes_value(true))
        .arg(Arg::with_name("panic_on_backup").short("P").long("panic")
                .help("Whether or not to panic on backup."))
        .arg(Arg::with_name("dry_run").short("d").long("dryrun")
                .help("Enabled 'dry run' aka only logging to STDOUT."))
}

/// Parses the arguments from the command line, and env
/// vars to get the final options hash. This is most likely
/// where users who don't know whats going on.
/// If a required option isn't passed through a command line option,
/// then it is assumed that it is provided through environment variable.
/// If it isn't passed through an environment variable then the program panics
/// because it can't unwrap a value.
pub fn get_args(matches: ArgMatches) -> Configuration {
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

    brokers = split_brokers(non_split_brokers);

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

    Configuration {
        cert_path: cert_path,
        key_path: key_path,
        brokers: brokers,
        port: port,
        panic_on_backup: panic_on_backup,
        dry_run: dry_run
    }
}

/// Takes in a String of comma seperated brokers,
/// and returns a Vector of IP:PORT. In the future this
/// will take care of DNS Lookup. However right now,
/// this simply splits based on the ',', and then resplits on
/// the ":" in order to make sure a user specified a port.
pub fn split_brokers(to_split: String) -> Vec<String> {
    to_split.split(',')
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
        .collect()
}

/// A function to resend all failed messages in JFS "DB".
/// This will only be called in a non-dry run state, and only when
/// the program is first booting.
pub fn resend_failed_messages(db: &Store, producer: Option<Arc<Mutex<Producer>>>) {
    let failed_to_sends = db.get_all::<MessagePayload>();

    if failed_to_sends.is_err() {
        println!("[-] Failed to get all failed to sends. Continuing.")
    }else {
        let failed_to_sends = failed_to_sends.unwrap();
        let producer = producer.unwrap();
        for (id, message_payload) in failed_to_sends.iter() {
            let cloned = message_payload.clone();

            let attempt_to_send = producer.lock().unwrap().send(&Record {
                topic: &cloned.topic,
                partition: -1,
                key: (),
                value: cloned.payload,
            });

            if attempt_to_send.is_err() {
                println!("[-] Failed to resend backup message: [ {:?} ]", message_payload.clone());
            } else {
                let _ = db.delete(&id);
            }
        }
        println!("[+] All done sending backup messages!");
    }
}

#[test]
fn test_configuration_parsing() {
    let matches = initialize_app().get_matches_from(vec![
        "kafka-proxy",
        "-b10.0.0.1:9092,10.0.0.2:9093",
        "-c/opt/place",
        "-k/opt/place2",
        "-p3000"
    ]);

    let config = get_args(matches);

    let test_cert_path: path::PathBuf = "/opt/place".to_string().into();
    let test_key_path: path::PathBuf = "/opt/place2".to_string().into();

    assert!(config.brokers == vec!["10.0.0.1:9092".to_string(), "10.0.0.2:9093".to_string()]);
    assert!(config.cert_path == test_cert_path);
    assert!(config.key_path == test_key_path);
    assert!(config.port == 3000);
    assert!(config.panic_on_backup == false);
    assert!(config.dry_run == false);
}

#[test]
fn test_flag_parsing() {
    let matches = initialize_app().get_matches_from(vec![
        "kafka-proxy",
        "-b10.0.0.1:9092,10.0.0.2:9093",
        "-c/opt/place",
        "-k/opt/place2",
        "-p3000",
        "-P",
        "-d"
    ]);

    let config = get_args(matches);

    let test_cert_path: path::PathBuf = "/opt/place".to_string().into();
    let test_key_path: path::PathBuf = "/opt/place2".to_string().into();

    assert!(config.brokers == vec!["10.0.0.1:9092".to_string(), "10.0.0.2:9093".to_string()]);
    assert!(config.cert_path == test_cert_path);
    assert!(config.key_path == test_key_path);
    assert!(config.port == 3000);
    assert!(config.panic_on_backup == true);
    assert!(config.dry_run == true);
}

#[test]
fn test_broker_split() {
    let example_brokers = "10.0.0.1:9092,10.0.0.1:9093,10.0.0.1:9095".to_string();

    assert!(split_brokers(example_brokers) == vec!["10.0.0.1:9092".to_string(),
        "10.0.0.1:9093".to_string(), "10.0.0.1:9095".to_string()]);
}

#[test]
#[should_panic]
fn test_invalid_broker_split() {
    let bad_brokers = "10.0.0.1,10.0.0.2".to_string();
    split_brokers(bad_brokers);
}
