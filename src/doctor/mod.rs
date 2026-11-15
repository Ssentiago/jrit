mod diagnose;
mod git;
mod github;
mod project;
mod report;
mod tags;

#[cfg(test)]
mod test_helpers;

pub use diagnose::diagnose;
pub use report::{CheckResult, CheckStatus, DoctorReport};
pub use tags::check_tag_available;
