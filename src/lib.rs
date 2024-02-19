pub mod config;
mod util;

use anyhow::anyhow;
use std::convert::TryFrom;

use chrono::{DateTime, NaiveDateTime, Utc};
use transmission_rpc::types::{ErrorType, TorrentGetField, TorrentStatus};
use url::Url;

/// A representation of a torrent on a transmission instance.
#[derive(PartialEq, Clone)]
pub struct Torrent {
    pub id: i64,
    pub hash: String,
    pub name: String,
    pub done_date: Option<DateTime<Utc>>,
    pub error: ErrorType,
    pub error_string: String,
    pub upload_ratio: f32,
    pub computed_upload_ratio: f64,
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
            .field("computed_upload_ratio", &self.computed_upload_ratio)
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
            UploadedEver,
            DoneDate,
            Files,
            TotalSize,
            Trackers,
        ])
    }

    /// Returns true of the torrent has no error status
    pub fn is_ok(&self) -> bool {
        self.error == ErrorType::Ok
    }
}

fn ensure_field<T>(field: Option<T>, name: &str) -> Result<T, anyhow::Error> {
    field.ok_or_else(|| anyhow!(format!("torrent has no field {name:?}")))
}

impl TryFrom<transmission_rpc::types::Torrent> for Torrent {
    type Error = anyhow::Error;

    fn try_from(t: transmission_rpc::types::Torrent) -> Result<Self, Self::Error> {
        let (uploaded_ever, total_size) = (
            ensure_field(t.uploaded_ever, "uploaded_ever")?,
            ensure_field(t.total_size, "total_size")?,
        );
        let computed_upload_ratio = uploaded_ever as f64 / total_size as f64;

        Ok(Torrent {
            id: ensure_field(t.id, "id")?,
            hash: ensure_field(t.hash_string, "hash_string")?,
            name: ensure_field(t.name, "name")?,
            done_date: t.done_date.and_then(|epoch| {
                NaiveDateTime::from_timestamp_opt(epoch, 0)
                    .map(|time| DateTime::from_naive_utc_and_offset(time, Utc))
            }),
            error: ensure_field(t.error, "error")?,
            error_string: ensure_field(t.error_string, "error_string")?,
            upload_ratio: ensure_field(t.upload_ratio, "upload_ratio")?,
            computed_upload_ratio,
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
