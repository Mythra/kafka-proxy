use std::path;

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
/// A Message Payload.
/// Used to simplify the passing of info from the HTTP Thread, to the thread that sends to Kafka.
/// Rather than using some weird string concatination method.
pub struct MessagePayload {
    pub topic: String,
    pub payload: String,
}

#[derive(Clone, Debug)]
/// The configuration struct.
/// Conatains all possible configuration values. Either from env vars,
/// or from CLI Opts.
pub struct Configuration {
    pub cert_path: path::PathBuf,
    pub key_path: path::PathBuf,
    pub brokers: Vec<String>,
    pub port: u64,
    pub panic_on_backup: bool,
    pub dry_run: bool,
}
