pub mod config;
mod util;

use anyhow::{anyhow, Context};
use std::convert::TryFrom;

use chrono::{DateTime, NaiveDateTime, Utc};
use enum_iterator::{self, Sequence};
use transmission_rpc::types::{TorrentGetField, TorrentStatus};
use url::Url;

/// What is this torrent doing right now?
#[derive(Sequence, PartialEq, Eq, Debug, Clone)]
pub enum Error {
    /// everything's fine
    Ok = 0,
    /// when we anounced to the tracker, we got a warning in the response
    TrackerWarning = 1,
    /// when we anounced to the tracker, we got an error in the response
    TrackerError = 2,
    /// local trouble, such as disk full or permissions error
    LocalError = 3,
}

impl TryFrom<i64> for Error {
    type Error = anyhow::Error;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value >= 0 && value <= (enum_iterator::last::<Error>().unwrap() as i64) {
            enum_iterator::all::<Error>()
                .nth(value as usize)
                .ok_or_else(|| anyhow!(format!("{value}")))
        } else {
            Err(anyhow!(format!("{value}")))
        }
    }
}

/// A representation of a torrent on a transmission instance.
#[derive(PartialEq, Clone)]
pub struct Torrent {
    pub id: i64,
    pub hash: String,
    pub name: String,
    pub done_date: Option<DateTime<Utc>>,
    pub error: Error,
    pub error_string: String,
    pub upload_ratio: f32,
    pub status: TorrentStatus,
    pub num_files: usize,
    pub total_size: usize,
    pub trackers: Vec<Url>,
}

impl std::fmt::Debug for Torrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let trackers: Vec<String> = self.trackers.iter().map(Url::to_string).collect();
        f.debug_struct("Torrent")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("done_date", &self.done_date)
            .field("error", &self.error)
            .field("error_string", &self.error_string)
            .field("upload_ratio", &self.upload_ratio)
            .field("status", &self.status)
            .field("num_files", &self.num_files)
            .field("total_size", &self.total_size)
            .field("trackers", &trackers)
            .finish()
    }
}

impl Torrent {
    pub fn request_fields() -> Option<Vec<TorrentGetField>> {
        use TorrentGetField::*;
        Some(vec![
            Id,
            HashString,
            Name,
            Error,
            ErrorString,
            Status,
            UploadRatio,
            DoneDate,
            Files,
            TotalSize,
            Trackers,
        ])
    }

    /// Returns true of the torrent has no error status
    pub fn is_ok(&self) -> bool {
        self.error == Error::Ok
    }
}

fn ensure_field<T>(field: Option<T>, name: &str) -> Result<T, anyhow::Error> {
    field.ok_or_else(|| anyhow!(format!("torrent has no field {name:?}")))
}

impl TryFrom<transmission_rpc::types::Torrent> for Torrent {
    type Error = anyhow::Error;

    fn try_from(t: transmission_rpc::types::Torrent) -> Result<Self, Self::Error> {
        Ok(Torrent {
            id: ensure_field(t.id, "id")?,
            hash: ensure_field(t.hash_string, "hash_string")?,
            name: ensure_field(t.name, "name")?,
            done_date: t.done_date.and_then(|epoch| {
                NaiveDateTime::from_timestamp_opt(epoch, 0)
                    .map(|time| DateTime::<Utc>::from_utc(time, Utc))
            }),
            error: Error::try_from(ensure_field(t.error, "error")?).context("parsing error")?,
            error_string: ensure_field(t.error_string, "error_string")?,
            upload_ratio: ensure_field(t.upload_ratio, "upload_ratio")?,
            status: ensure_field(t.status, "status")?,
            num_files: ensure_field(t.files, "files")?.len(),
            total_size: ensure_field(t.total_size, "total_size")? as usize,
            trackers: ensure_field(t.trackers, "trackers")?
                .into_iter()
                .map(|t| Url::parse(&t.announce))
                .collect::<Result<Vec<Url>, url::ParseError>>()?,
        })
    }
}
