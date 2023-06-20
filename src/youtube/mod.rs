use url::Url;

pub mod channel;
pub mod video;

/// Parses a YouTube URL and returns its details. Returns an Err if the URL
/// could not be parsed, or if it's not a supported URL. Supported URLs are:
///
/// - Videos (youtube.com/watch?v=... and youtu.be/...)
/// - Channels (youtube.com/channel/..., youtube.com/c/...)
/// - Playlists (youtube.com/playlist?list=...)
#[derive(Debug)]
pub struct URL {
    parsed_uri: Url,
}

#[derive(Debug)]
pub enum URLParseError {
    InvalidUri(url::ParseError),
    UnsupportedUri,
}

impl std::fmt::Display for URLParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            URLParseError::InvalidUri(e) => write!(f, "Invalid URI: {}", e),
            URLParseError::UnsupportedUri => write!(f, "Unsupported URI"),
        }
    }
}

impl URL {
    pub fn parse(s: &str) -> Result<Self, URLParseError> {
        let uri = Url::parse(s).map_err(|e| URLParseError::InvalidUri(e))?;
        let host = uri.host().ok_or(URLParseError::UnsupportedUri)?.to_string();

        // Make sure it's youtube.com
        if ["www.youtube.com", "youtube.com", "youtu.be"]
            .iter()
            .all(|&d| d != host)
        {
            return Err(URLParseError::UnsupportedUri);
        }

        Ok(URL { parsed_uri: uri })
    }

    pub fn video_id(&self) -> Option<String> {
        let host = self.parsed_uri.host()?.to_string();
        let path = self.parsed_uri.path();

        if host == "youtu.be" {
            return Some(path[1..].to_string());
        }

        if path.starts_with("/live/") {
            return Some(path[6..].to_string());
        }

        if path == "/watch" {
            return self
                .parsed_uri
                .query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.to_string());
        }

        None
    }

    pub fn channel_id(&self) -> Option<String> {
        let mut segs = self.parsed_uri.path_segments()?;

        match segs.next()? {
            "channel" => Some(segs.next()?.to_string()),
            _ => None,
        }
    }

    pub fn channel_vanity(&self) -> Option<String> {
        let mut segs = self.parsed_uri.path_segments()?;

        match segs.next()? {
            "c" => Some(segs.next()?.to_string()),
            _ => None,
        }
    }

    pub fn playlist_id(&self) -> Option<String> {
        return self
            .parsed_uri
            .query_pairs()
            .find(|(k, _)| k == "list")
            .map(|(_, v)| v.to_string());
    }
}

impl TryFrom<&str> for URL {
    type Error = URLParseError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        URL::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::URL;

    #[test]
    fn test_parse_url() {
        URL::parse("meow nyan").expect_err("Should not parse");
        URL::parse("https://random.website").expect_err("Should be unsupported");
        assert_eq!(
            URL::parse("https://youtu.be/IKKar5SS29E")
                .expect("Should parse")
                .video_id(),
            Some("IKKar5SS29E".into()),
        );
        assert_eq!(
            URL::parse("https://youtube.com/watch?v=stmZAThUl64&blah=1")
                .expect("Should parse")
                .video_id(),
            Some("stmZAThUl64".into()),
        );
        assert_eq!(
            URL::parse("https://www.youtube.com/watch?asdf=2&v=8ZdLXELdF9Q")
                .expect("Should parse")
                .video_id(),
            Some("8ZdLXELdF9Q".into()),
        );
        assert_eq!(
            URL::parse("https://www.youtube.com/channel/UCjLEmnpCNeisMxy134KPwWw")
                .expect("Should parse")
                .channel_id(),
            Some("UCjLEmnpCNeisMxy134KPwWw".into()),
        );
        assert_eq!(
            URL::parse("https://www.youtube.com/c/loudnessfete")
                .expect("Should parse")
                .channel_vanity(),
            Some("loudnessfete".into()),
        );
    }
}
