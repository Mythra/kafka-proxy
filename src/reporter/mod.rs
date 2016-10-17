use std::thread;
use std::sync::mpsc::{Sender};
use std::sync::{Arc, Mutex, mpsc};

#[cfg(feature = "reporter-slack")]
use std::env;

#[cfg(feature = "reporter-slack")]
lazy_static! {
    static ref SLACK_WEBHOOK: String = env::var("SLACK_WEBHOOK").unwrap();
    static ref SLACK_CHANNEL: String = env::var("SLACK_CHANNEL").unwrap_or("#general".to_string());
}

/// A Failure Reporter.
/// This reports failures to a specific source. The source is defined by features,
/// that are enabled at build time. Right now the only two reporters are the
/// "NoOp" reporter which is the default, and only logs to STDOUT, and the
/// "reporter-slack", which will post to Slack when a message fails to send.
/// Simply create a reporter instance, and call the function "start_reporting".
/// That will return a mpsc Sender which has been wrapped with a mutex + arc so
/// it can be cloned, and is thread safe.
pub struct Reporter;

#[cfg(feature = "reporter-slack")]
impl Reporter {
    /// Starts the Slack Reporter Thread. Creates a mpsc channel,
    /// reads from the environment variable "SLACK_WEBHOOK",
    /// and "SLACK_CHANNEL" (which defaults to "#general").
    /// Then spawns a thread, and returns an Arc<Mutex<Sender>>.
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<()>>> {
        use slack_hook::{Slack, AttachmentBuilder, PayloadBuilder};

        let (tx, rx) = mpsc::channel::<()>();
        let slack = Slack::new(&SLACK_WEBHOOK[..]);
        if slack.is_err() {
            panic!("Failed to setup slack client.");
        }
        let slack = slack.unwrap();
        info!("Starting Slack Reporter...");
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
    /// Starts the NoOp Reporter.
    /// Creates an mpsc Channel, spins up a thread, and
    /// returns the Sender wrapped in a mutex + arc.
    pub fn start_reporting(&self) -> Arc<Mutex<Sender<()>>> {
        let (tx, rx) = mpsc::channel::<()>();
        info!("Starting NoOp Reporter...");
        thread::spawn(move || {
            loop {
                let possible_failure = rx.try_recv();
                if possible_failure.is_ok() {
                    debug!("We could've reported to somewhere that this failed. But it's not configured.");
                }
            }
        });
        Arc::new(Mutex::new(tx))
    }
}
