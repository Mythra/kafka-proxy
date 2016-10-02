use std::thread;
use std::sync::mpsc::{Sender};
use std::sync::{Arc, Mutex, mpsc};

#[cfg(feature = "reporter-slack")]
use std::env;
#[cfg(feature = "reporter-slack")]
use slack_hook::{Slack, AttachmentBuilder, PayloadBuilder};

#[cfg(feature = "reporter-slack")]
lazy_static! {
    static ref SLACK_WEBHOOK: String = env::var("SLACK_WEBHOOK").unwrap();
    static ref SLACK_CHANNEL: String = env::var("SLACK_CHANNEL").unwrap_or("#general".to_string());
}

pub struct Reporter {}

#[cfg(feature = "reporter-slack")]
impl Reporter {
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<()>>> {
        let (tx, rx) = mpsc::channel::<()>();
        let slack = Slack::new(&SLACK_WEBHOOK[..]);
        if slack.is_err() {
            panic!("Failed to setup slack client.");
        }
        let slack = slack.unwrap();
        println!("Starting Slack Reporter...");
        thread::spawn(move || {
            loop {
                let possible_failure = rx.try_recv();
                if possible_failure.is_ok() {
                    let p = PayloadBuilder::new()
                        .channel((*SLACK_CHANNEL).clone())
                        .username("Kafka Reporter")
                        .icon_emoji(":apache-kafka:")
                        .attachments(vec![
                            AttachmentBuilder::new("Failed to Send to Kafka! :cry:")
                                .color("danger").build().unwrap()
                        ])
                        .build()
                        .unwrap();

                    let _ = slack.send(&p);
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}

#[cfg(all(not(feature = "reporter-slack")))]
impl Reporter {
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<()>>> {
        let (tx, rx) = mpsc::channel::<()>();
        println!("Starting NoOp Reporter...");
        thread::spawn(move || {
            loop {
                let possible_failure = rx.try_recv();
                if possible_failure.is_ok() {
                    println!("[-] We could've reported to somewhere that this failed. But it's not configured.");
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}
