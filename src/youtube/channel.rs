use anyhow::{anyhow, Context, Result};
use lazy_static::lazy_static;
use reqwest::Client;

pub async fn fetch_picture_url(client: Client, channel_id: &str) -> Result<String> {
    // Fetch the channel page
    let channel_url = format!("https://www.youtube.com/channel/{}", channel_id);
    let res = client
        .get(&channel_url)
        .send()
        .await
        .context("Failed to fetch channel page")?
        .text()
        .await
        .context("Failed to read channel page")?;

    // Find the picture URL
    lazy_static! {
        static ref RE: regex::Regex =
            regex::Regex::new(r#"<meta name="twitter:image" content="(.*?)""#).unwrap();
    }
    let captures = RE
        .captures(&res)
        .ok_or_else(|| anyhow!("Could not find picture URL"))?;
    let picture_url = captures
        .get(1)
        .ok_or_else(|| anyhow!("Could not find picture URL"))?
        .as_str();
    debug!("[{}] Found picture URL: {}", channel_id, picture_url);

    Ok(picture_url.to_owned())
}
