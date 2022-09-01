use actix_web::http::Uri;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct InitialPlayerResponse {
    #[serde(rename = "videoDetails")]
    pub video_details: InitialPlayerResponseVideoDetails,
}
#[derive(Deserialize)]
pub struct InitialPlayerResponseVideoDetails {
    #[serde(rename = "videoId")]
    pub video_id: String,
    pub title: String,
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub author: String,
    pub thumbnail: InitialPlayerResponseVideoDetailsThumbnail,
}
#[derive(Deserialize)]
pub struct InitialPlayerResponseVideoDetailsThumbnail {
    pub thumbnails: Vec<InitialPlayerResponseVideoDetailsThumbnailThumbnail>,
}
#[derive(Deserialize)]
pub struct InitialPlayerResponseVideoDetailsThumbnailThumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

pub async fn fetch_initial_player_response(
    client: Client,
    url: &str,
) -> Result<InitialPlayerResponse> {
    // Parse URL
    let uri = url
        .parse::<Uri>()
        .map_err(|e| anyhow!("Invalid URL: {}", e))?;

    // Make sure it's a supported URL
    let host = uri.host().ok_or(anyhow!("Invalid URL"))?;
    if host != "youtube.com" && host != "www.youtube.com" && host != "youtu.be" {
        return Err(anyhow!("Unsupported URL"));
    }

    // Fetch the video URL
    let html = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to fetch video URL: {}", e))?
        .error_for_status()
        .map_err(|e| anyhow!("Failed to fetch video URL: {}", e))?
        .text()
        .await
        .map_err(|e| anyhow!("Failed to fetch video URL: {}", e))?;

    // Parse page contents
    lazy_static::lazy_static! {
        static ref IPR_RE: regex::Regex =
            regex::Regex::new(r#"ytInitialPlayerResponse = (.*?});"#).unwrap();
    }

    let ipr = IPR_RE
        .captures(&html)
        .ok_or(anyhow!("Failed to find the initial player response"))?
        .get(1)
        .ok_or(anyhow!("Failed to find the initial player response"))?
        .as_str();

    // Parse the initial player response
    let ipr: InitialPlayerResponse = serde_json::from_str(ipr)
        .map_err(|e| anyhow!("Failed to parse the initial player response: {}", e))?;

    Ok(ipr)
}
