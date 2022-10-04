use std::{borrow::Cow, collections::HashSet, fmt};

use crate::util::chrono_optional_duration;
use chrono::{Duration, Utc};
use rhai::{Array, Dynamic, EvalAltResult};
use serde::{Deserialize, Serialize};
use tracing::debug;
use url::Url;

use crate::Torrent;

/// A set of conditions that indicate that a torrent is governed by a particular policy.
///
/// The policy itself doesn't need to match, this is just to indicate
/// that it *could* even match.
#[derive(PartialEq, Eq, Clone, Default, Debug, Serialize, Deserialize)]
pub struct PolicyMatch {
    /// The tracker URL hostnames (only the host, not the path or
    /// port) that the policy should apply to.
    pub trackers: HashSet<String>,

    /// The number of files that must be present in a torrent for the
    /// policy to match. If None, any number of files matches.
    pub min_file_count: Option<i64>,

    /// The maximum number of files that may be present in a torrent
    /// for the policy to match. If None, any number of files matches.
    pub max_file_count: Option<i64>,
}

impl PolicyMatch {
    pub fn new(trackers: Array) -> Result<Self, Box<EvalAltResult>> {
        let trackers: Vec<String> = Dynamic::from(trackers).into_typed_array()?;
        Ok(PolicyMatch {
            trackers: trackers.into_iter().collect(),
            ..Default::default()
        })
    }

    pub fn with_min_file_count(self, min_file_count: i64) -> Self {
        Self {
            min_file_count: Some(min_file_count),
            ..self
        }
    }

    pub fn with_max_file_count(self, max_file_count: i64) -> Self {
        Self {
            max_file_count: Some(max_file_count),
            ..self
        }
    }

    #[tracing::instrument]
    fn governed_by_policy(&self, t: &Torrent) -> bool {
        if t.status != crate::Status::Seeding {
            debug!("Torrent {:?} is not seeding, bailing", t);
            return false;
        }

        if !t
            .trackers
            .iter()
            .filter_map(Url::host_str)
            .any(|tracker_host| self.trackers.contains(tracker_host))
        {
            debug!(
                "Torrent {:?} does not have matching trackers, expected {:?}",
                t, self.trackers
            );
            return false;
        }

        let file_count = t.num_files as i64;
        match (self.min_file_count, self.max_file_count) {
            (Some(min), Some(max)) if file_count < min || file_count > max => {
                debug!(
                    "Torrent {:?} doesn't have the right number of files: {}",
                    t, file_count
                );
                return false;
            }
            (None, Some(max)) if file_count > max => {
                debug!(
                    "Torrent {:?} doesn't have the right number of files: {}",
                    t, file_count
                );
                return false;
            }
            (Some(min), None) if file_count < min => {
                debug!(
                    "Torrent {:?} doesn't have the right number of files: {}",
                    t, file_count
                );
                return false;
            }
            (_, _) => {}
        }

        true
    }
}

impl fmt::Display for PolicyMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pre:[{:?}", self.trackers)?;
        if let Some(min_file_count) = self.min_file_count {
            write!(f, " {}<f", min_file_count)?;
            if let Some(max_file_count) = self.max_file_count {
                write!(f, "<={}", max_file_count)?;
            }
        } else if let Some(max_file_count) = self.max_file_count {
            write!(f, " f<={}", max_file_count)?;
        }
        write!(f, "]")
    }
}

/// Conditions for matching a torrent that are governed by a policy on
/// a transmission instance.
///
/// There's a second set of conditions that need to match: See [PolicyMatch].
#[derive(PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct Condition {
    /// The ratio at which a torrent qualifies for deletion, even if
    /// it has been seeded for less than [`max_seeding_time`].
    pub max_ratio: Option<f64>,

    /// The minimum amount of time that a torrent must have been
    /// seeding for, to qualify for deletion.
    ///
    /// Even if the [`max_ratio`] requirement isn't met, the torrent
    /// won't be deleted unless it's been seeding this long.
    #[serde(with = "chrono_optional_duration")]
    pub min_seeding_time: Option<Duration>,

    /// The duration at which a torrent qualifies for deletion.
    #[serde(with = "chrono_optional_duration")]
    pub max_seeding_time: Option<Duration>,
}

impl Condition {
    pub fn new() -> Result<Self, Box<EvalAltResult>> {
        Ok(Condition {
            ..Default::default()
        })
    }

    pub fn with_min_seeding_time(self, min_seeding_time: &str) -> Result<Self, Box<EvalAltResult>> {
        let min_seeding_time = Some(
            Duration::from_std(
                parse_duration::parse(min_seeding_time).map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?,
        );
        Ok(Self {
            min_seeding_time,
            ..self
        })
    }

    pub fn with_max_seeding_time(self, max_seeding_time: &str) -> Result<Self, Box<EvalAltResult>> {
        let max_seeding_time = Some(
            Duration::from_std(
                parse_duration::parse(max_seeding_time).map_err(|e| format!("{e}"))?,
            )
            .map_err(|e| format!("{e}"))?,
        );
        Ok(Self {
            max_seeding_time,
            ..self
        })
    }

    pub fn with_max_ratio(self, max_ratio: f64) -> Self {
        Self {
            max_ratio: Some(max_ratio),
            ..self
        }
    }
}

mod condition_match {
    #![allow(clippy::extra_unused_lifetimes)]

    use chrono::Duration;
    use enum_kinds::EnumKind;

    #[derive(PartialEq, Copy, Clone, Debug, EnumKind)]
    #[enum_kind(ConditionMatchKind)]
    pub enum ConditionMatch {
        /// Preconditions met, but did not match.
        None,

        /// Matches based on ratio
        Ratio(f64),

        /// Matches based on seed time
        SeedTime(Duration),
    }
}
pub use condition_match::*;

impl fmt::Display for ConditionMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use hhmmss::Hhmmss;
        use ConditionMatch::*;
        match self {
            None => write!(f, "None"),
            Ratio(r) => write!(f, "Ratio({})", r),
            SeedTime(d) => write!(f, "SeedTime({})", d.hhmmss()),
        }
    }
}

impl ConditionMatch {
    pub fn is_match(&self) -> bool {
        self != &ConditionMatch::None
    }

    pub fn is_real_mismatch(&self) -> bool {
        self == &ConditionMatch::None
    }
}

impl Condition {
    pub fn sanity_check(self) -> Result<Self, Box<EvalAltResult>> {
        if vec![
            self.min_seeding_time.map(|_| true),
            self.max_ratio.map(|_| true),
            self.max_seeding_time.map(|_| true),
        ]
        .iter()
        .all(Option::is_none)
        {
            Err("Set at least one of min_seeding_time, max_seeding_time, max_ratio - otherwise this deletes all torrents matching the tracker immediately.".to_string())?;
        }
        Ok(self)
    }

    /// Returns true if the condition matches a given torrent.
    #[tracing::instrument]
    pub fn matches_torrent(&self, t: &Torrent) -> ConditionMatch {
        if let Some(done_date) = t.done_date {
            if done_date.timestamp() == 0 {
                // Can never be a useful time
                debug!("Unset 'done' time on {:?} - leaving it alone", t);
                return ConditionMatch::None;
            }
            let seed_time = Utc::now() - done_date;

            if let Some(min_seeding_time) = self.min_seeding_time {
                if seed_time < min_seeding_time {
                    debug!("Torrent {:?} doesn't meet the min seeding time reqs yet", t);
                    return ConditionMatch::None;
                }
            }

            if let Some(max_ratio) = self.max_ratio {
                if t.upload_ratio as f64 >= max_ratio {
                    debug!(
                        "Torrent {:?} has a ratio that qualifies it for deletion'",
                        t
                    );
                    return ConditionMatch::Ratio(t.upload_ratio as f64);
                }
            }
            if let Some(max_seeding_time) = self.max_seeding_time {
                if seed_time >= max_seeding_time {
                    debug!("Torrent {:?} matches seed time requirements", t);
                    return ConditionMatch::SeedTime(seed_time);
                }
            }
        }
        ConditionMatch::None
    }
}

impl fmt::Debug for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "When:[")?;
        if let Some(min_seeding_time) = self.min_seeding_time {
            write!(f, " {}>t", min_seeding_time)?;
            if let Some(max_seeding_time) = self.max_seeding_time {
                write!(f, "<={}", max_seeding_time)?;
            }
        } else if let Some(max_seeding_time) = self.max_seeding_time {
            write!(f, " t<={}", max_seeding_time)?;
        }
        if let Some(max_ratio) = self.max_ratio {
            write!(f, " r<{}", max_ratio)?;
        }
        write!(f, "]")
    }
}

/// A policy that can be applied to a given torrent.
#[derive(Debug, PartialEq)]
pub struct ApplicableDeletePolicy<'a> {
    torrent: &'a Torrent,
    policy: &'a DeletePolicy,
}

impl<'a> ApplicableDeletePolicy<'a> {
    /// Checks whether the torrent can be deleted.
    pub fn matches(&self) -> ConditionMatch {
        self.policy.match_when.matches_torrent(self.torrent)
    }
}

/// Specifies a condition for torrents that can be deleted.
#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub struct DeletePolicy {
    pub name: Option<String>,

    /// The condition under which a torrent is governed by this policy.
    pub(crate) precondition: PolicyMatch,

    /// The condition indicating whether to delete a governed torrent.
    #[serde(rename = "match")]
    pub(crate) match_when: Condition,

    /// Whether to pass "trash data" to the transmission API method.
    pub delete_data: bool,
}

impl DeletePolicy {
    /// Ensures that the policy can be applied to a torrent, and only
    /// if it is, allows chaining a `.matches` call.
    pub fn applicable<'a>(&'a self, t: &'a Torrent) -> Option<ApplicableDeletePolicy> {
        self.precondition
            .governed_by_policy(t)
            .then_some(ApplicableDeletePolicy {
                torrent: t,
                policy: self,
            })
    }
}

impl fmt::Debug for DeletePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl fmt::Display for DeletePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DeletePolicy:[{:?}, {}, delete_data:{}]",
            self.name, self.match_when, self.delete_data
        )
    }
}

impl DeletePolicy {
    pub fn name_or_index(&self, index: usize) -> Cow<String> {
        self.name
            .as_ref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(index.to_string()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    // Should never delete younglings:
    #[test_case("1 min", 0.0, Some(ConditionMatchKind::None); "young torrent at unmet ratio")]
    #[test_case("1 min", 7.0, Some(ConditionMatchKind::None); "young torrent at exceeded ratio")]
    // If they're older, we can delete if ratio is met:
    #[test_case("6 hrs", 1.1, Some(ConditionMatchKind::Ratio); "medium and ratio exceeded")]
    #[test_case("6 hrs", 0.9, Some(ConditionMatchKind::None); "medium and ratio not met")]
    // Any that are really old are fair game:
    #[test_case("12 days", 0.9, Some(ConditionMatchKind::SeedTime); "when seeding long enough at unmet ratio")]
    #[test_case("12 days", 1.5, Some(ConditionMatchKind::Ratio); "when seeding long enough at exceeded ratio")]
    #[test_log::test]
    fn condition_seed_time(time: &str, upload_ratio: f32, matches: Option<ConditionMatchKind>) {
        let time = Duration::from_std(parse_duration::parse(time).unwrap()).unwrap();
        let precondition = PolicyMatch {
            trackers: vec!["tracker".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let match_when = Condition {
            max_ratio: Some(1.0),
            min_seeding_time: Some(Duration::minutes(60)),
            max_seeding_time: Some(Duration::days(2)),
            ..Default::default()
        };
        let pol = DeletePolicy {
            name: None,
            precondition,
            match_when,
            delete_data: false,
        };
        let t = Torrent {
            id: 1,
            hash: "abcd".to_string(),
            name: "testcase".to_string(),
            done_date: Some(Utc::now() - time),
            error: crate::Error::Ok,
            error_string: "".to_string(),
            upload_ratio,
            status: crate::Status::Seeding,
            num_files: 1,
            total_size: 30000,
            trackers: vec![Url::parse("https://tracker:8080/announce").unwrap()],
        };
        assert_eq!(
            pol.applicable(&t)
                .map(|a| a.matches())
                .map(ConditionMatchKind::from),
            matches
        );
    }

    #[test_case(1, true; "single-file torrent")]
    #[test_case(2, false; "within range: 2")]
    #[test_case(3, false; "within range: 3")]
    #[test_case(4, false; "within range: 4")]
    #[test_case(5, true; "out of range: 5")]
    #[test_log::test]
    fn condition_num_files(num_files: usize, rejected: bool) {
        let precondition = PolicyMatch {
            trackers: vec!["tracker".to_string()].into_iter().collect(),
            min_file_count: Some(2),
            max_file_count: Some(4),
        };
        let match_when = Condition {
            max_ratio: Some(1.0),
            min_seeding_time: Some(Duration::minutes(60)),
            max_seeding_time: Some(Duration::days(2)),
            ..Default::default()
        };
        let pol = DeletePolicy {
            match_when,
            precondition,
            name: None,
            delete_data: false,
        };
        let t = Torrent {
            id: 1,
            hash: "abcd".to_string(),
            name: "testcase".to_string(),
            done_date: Some(Utc::now() - Duration::days(12)),
            error: crate::Error::Ok,
            error_string: "".to_string(),
            upload_ratio: 2.0,
            status: crate::Status::Seeding,
            num_files,
            total_size: 30000,
            trackers: vec![Url::parse("https://tracker:8080/announce").unwrap()],
        };
        if rejected {
            assert_eq!(pol.applicable(&t).map(|a| a.matches()), None);
        } else {
            assert_ne!(pol.applicable(&t).map(|a| a.matches()), None);
        }
    }

    #[test_case("http://example.com:8080/announce", false; "with tracker that matches")]
    #[test_case(
        "http://example-nomatch.com:8080/announce",
        true;
        "with tracker that does not match"
    )]
    #[test_log::test]
    fn tracker_url(tracker: &str, rejected: bool) {
        let precondition = PolicyMatch {
            trackers: vec!["example.com".to_string()].into_iter().collect(),
            min_file_count: Some(2),
            max_file_count: Some(4),
        };
        let match_when = Condition {
            max_ratio: Some(1.0),
            min_seeding_time: Some(Duration::minutes(60)),
            max_seeding_time: Some(Duration::days(2)),
            ..Default::default()
        };
        let pol = DeletePolicy {
            match_when,
            precondition,
            name: None,
            delete_data: false,
        };
        let t = Torrent {
            id: 1,
            hash: "abcd".to_string(),
            name: "testcase".to_string(),
            done_date: Some(Utc::now() - Duration::days(12)),
            error: crate::Error::Ok,
            error_string: "".to_string(),
            upload_ratio: 2.0,
            status: crate::Status::Seeding,
            num_files: 3,
            total_size: 30000,
            trackers: vec![Url::parse(tracker).unwrap()],
        };
        if rejected {
            assert_eq!(pol.applicable(&t).map(|a| a.matches()), None);
        } else {
            assert_ne!(pol.applicable(&t).map(|a| a.matches()), None);
        }
    }
}
