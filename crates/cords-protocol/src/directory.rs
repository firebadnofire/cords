//! Authenticated server membership presentation, separate from device discovery.
use crate::messaging::Contact;
use serde::{Deserialize, Serialize};

/// A server-admitted account. Device cards are legacy review candidates, not a
/// canonical account profile; consumers must not pick a conflicting candidate.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryAccount {
    pub account_id: String,
    pub devices: Vec<Contact>,
}

/// HTTPS-authenticated snapshot. Pagination does not promise a frozen directory
/// across requests; refresh from the beginning after membership changes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccountDirectoryPage {
    pub version: u16,
    pub server_id: String,
    pub accounts: Vec<DirectoryAccount>,
    pub next_after: Option<String>,
}
