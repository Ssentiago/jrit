use super::CheckResult;
use crate::config::Config;
use crate::pipeline::pipeline_steps::github_step;

pub(super) async fn check_github_auth(config: &Config) -> CheckResult {
    let token = match github_step::get_token().await {
        Ok(token) => token,
        Err(e) => {
            return CheckResult::fail("github auth", format!("no GitHub credentials: {e:#}"));
        }
    };

    let url = format!(
        "https://api.github.com/repos/{}/{}",
        config.project.owner(),
        config.project.repo_name()
    );
    let client = reqwest::Client::new();
    match client
        .get(&url)
        .bearer_auth(&token)
        .header("User-Agent", "jrit")
        .send()
        .await
    {
        Ok(response) => classify_auth_status(response.status().as_u16()),
        Err(e) => CheckResult::fail("github auth", format!("cannot reach api.github.com: {e}")),
    }
}

/// Maps a GitHub API response code to a check result, kept pure so it can be
/// tested without network access.
fn classify_auth_status(status: u16) -> CheckResult {
    match status {
        200..=299 => CheckResult::ok(
            "github auth",
            "the token works and the repository is accessible",
        ),
        401 => CheckResult::fail("github auth", "the token is invalid or expired"),
        403 => CheckResult::fail(
            "github auth",
            "the token has no access to the repository, or the API rate limit is exhausted",
        ),
        404 => CheckResult::fail(
            "github auth",
            "the repository was not found, or the token cannot see it",
        ),
        other => CheckResult::fail(
            "github auth",
            format!("unexpected response from the GitHub API: HTTP {other}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::CheckStatus;

    #[test]
    fn classify_auth_status_maps_http_codes() {
        assert_eq!(classify_auth_status(200).status, CheckStatus::Ok);
        assert_eq!(classify_auth_status(204).status, CheckStatus::Ok);

        let unauthorized = classify_auth_status(401);
        assert_eq!(unauthorized.status, CheckStatus::Fail);
        assert!(unauthorized.message.contains("invalid or expired"));

        let forbidden = classify_auth_status(403);
        assert_eq!(forbidden.status, CheckStatus::Fail);
        assert!(forbidden.message.contains("no access"));

        let missing = classify_auth_status(404);
        assert_eq!(missing.status, CheckStatus::Fail);
        assert!(missing.message.contains("not found"));

        let broken = classify_auth_status(500);
        assert_eq!(broken.status, CheckStatus::Fail);
        assert!(broken.message.contains("HTTP 500"));
    }
}
