use crate::doctor;
use owo_colors::OwoColorize;

pub async fn run_doctor() -> anyhow::Result<()> {
    let report = doctor::diagnose(false).await;
    print!("{}", report.render());

    if report.has_failures() {
        anyhow::bail!("{} of {} checks failed", report.failure_count(), report.checks.len());
    }

    match report.warning_count() {
        0 => println!("{} all checks passed", "✓".green()),
        n => println!("{} all checks passed with {n} warnings", "!".yellow().bold()),
    }
    Ok(())
}
