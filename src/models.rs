use std::path;

#[derive(Clone, Debug, RustcEncodable, RustcDecodable)]
pub struct MessagePayload {
    pub topic: String,
    pub payload: String,
}

#[derive(Clone, Debug)]
pub struct Configuration {
    pub cert_path: path::PathBuf,
    pub key_path: path::PathBuf,
    pub brokers: Vec<String>,
    pub port: u64,
    pub panic_on_backup: bool,
    pub dry_run: bool,
}
