use std::time::Duration;

use protocol::{CheckForUpdateRequest, CheckForUpdateResult};
use reqwest::blocking::Client;
use serde::Deserialize;

use super::{HandlerError, HandlerResult};

const RELEASES_API_URL: &str =
    "https://api.github.com/repos/bukkumaaku/auto-tagger-eagle-plugin/releases/latest";
const RELEASES_PAGE_URL: &str =
    "https://github.com/bukkumaaku/auto-tagger-eagle-plugin/releases/latest";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

pub fn handle(_request: CheckForUpdateRequest) -> HandlerResult<CheckForUpdateResult> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("eagle-auto-tagger/{current_version}"))
        .build()
        .map_err(|error| HandlerError::new("UPDATE_CHECK_FAILED", error.to_string()))?;

    let response = client
        .get(RELEASES_API_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|error| HandlerError::new("UPDATE_CHECK_FAILED", error.to_string()))?;

    if !response.status().is_success() {
        return Err(HandlerError::new(
            "UPDATE_CHECK_FAILED",
            format!("GitHub Releases returned HTTP {}", response.status()),
        ));
    }

    let release = response
        .json::<GithubRelease>()
        .map_err(|error| HandlerError::new("UPDATE_CHECK_FAILED", error.to_string()))?;
    let latest_version = normalize_version(&release.tag_name).ok_or_else(|| {
        HandlerError::new(
            "UPDATE_CHECK_FAILED",
            format!("invalid release version: {}", release.tag_name),
        )
    })?;
    let current_parts = version_parts(&current_version).ok_or_else(|| {
        HandlerError::new(
            "UPDATE_CHECK_FAILED",
            format!("invalid current version: {current_version}"),
        )
    })?;
    let latest_parts = version_parts(&latest_version).expect("normalized release version");

    Ok(CheckForUpdateResult {
        current_version,
        latest_version,
        update_available: latest_parts > current_parts,
        release_url: if release.html_url.is_empty() {
            RELEASES_PAGE_URL.to_string()
        } else {
            release.html_url
        },
    })
}

fn normalize_version(value: &str) -> Option<String> {
    let value = value.trim().trim_start_matches(['v', 'V']);
    let version = value.split_once('-').map_or(value, |(version, _)| version);
    version_parts(version).map(|_| version.to_string())
}

fn version_parts(value: &str) -> Option<[u64; 3]> {
    let mut parts = value.split('.').map(str::parse::<u64>);
    let result = [
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ];
    if parts.next().is_some() {
        return None;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_release_tags() {
        assert_eq!(normalize_version("v2.0.1"), Some("2.0.1".to_string()));
        assert_eq!(normalize_version("V2.2.0-beta"), Some("2.2.0".to_string()));
        assert_eq!(normalize_version("release"), None);
    }

    #[test]
    fn compares_three_part_versions() {
        assert!(version_parts("2.0.1") > version_parts("2.0.0"));
        assert_eq!(version_parts("2.0"), None);
    }
}
