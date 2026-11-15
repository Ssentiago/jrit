use owo_colors::OwoColorize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Fail,
    Skipped,
}

pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

impl CheckResult {
    pub fn ok(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Ok,
            message: message.into(),
        }
    }

    pub fn fail(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            message: message.into(),
        }
    }

    pub fn warning(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warning,
            message: message.into(),
        }
    }

    pub fn skipped(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Skipped,
            message: message.into(),
        }
    }
}

pub struct DoctorReport {
    pub checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub fn failure_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Fail)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|check| check.status == CheckStatus::Warning)
            .count()
    }

    pub fn has_failures(&self) -> bool {
        self.failure_count() > 0
    }

    pub fn render(&self) -> String {
        let mut out = String::from("Preflight checks:\n");
        for check in &self.checks {
            let (icon, name) = match check.status {
                CheckStatus::Ok => ("✓".green().to_string(), check.name.green().to_string()),
                CheckStatus::Fail => ("✗".red().to_string(), check.name.red().to_string()),
                CheckStatus::Warning => ("!".yellow().to_string(), check.name.yellow().to_string()),
                CheckStatus::Skipped => ("-".dimmed().to_string(), check.name.dimmed().to_string()),
            };
            let message = check.message.replace('\n', "\n    ");
            out.push_str(&format!("  {icon} {name}: {message}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_counts_failures() {
        let report = DoctorReport {
            checks: vec![
                CheckResult::ok("a", "fine"),
                CheckResult::warning("b", "meh"),
                CheckResult::fail("c", "bad"),
                CheckResult::skipped("d", "off"),
            ],
        };
        assert!(report.has_failures());
        assert_eq!(report.failure_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert!(report.render().contains("Preflight checks"));
    }

    #[test]
    fn warnings_alone_do_not_fail_the_report() {
        let report = DoctorReport {
            checks: vec![
                CheckResult::ok("a", "fine"),
                CheckResult::warning("b", "meh"),
            ],
        };
        assert!(!report.has_failures());
        assert_eq!(report.warning_count(), 1);
    }
}
