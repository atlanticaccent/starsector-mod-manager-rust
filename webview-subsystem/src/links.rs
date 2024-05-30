mod drive {
  use std::sync::OnceLock;

  use regex::Regex;

  fn extract_google_drive_id(url: &str) -> Option<&str> {
    // Define regex patterns for different Google Drive link formats
    static PATTERNS: OnceLock<[Regex; 5]> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
      [
        Regex::new(r"https://drive\.google\.com/file/d/([^/]+)/?").unwrap(),
        Regex::new(r"https://drive\.google\.com/open\?id=([^&]+)").unwrap(),
        Regex::new(r"https://drive\.google\.com/uc\?id=([^&]+)").unwrap(),
        Regex::new(r"https://drive\.google\.com/a/[^/]+/file/d/([^/]+)/?").unwrap(),
        Regex::new(r"https://drive\.google\.com/drive/folders/([^/]+)/?").unwrap(),
      ]
    });

    // Iterate over the patterns and try to capture the ID
    for pattern in patterns.iter() {
      if let Some(captures) = pattern.captures(url) {
        if let Some(id) = captures.get(1) {
          return Some(id.as_str());
        }
      }
    }

    // Return None if no pattern matched
    None
  }

  pub(super) fn convert_to_direct_download_link(url: &str) -> Option<String> {
    // Extract the Google Drive ID
    if let Some(id) = extract_google_drive_id(url) {
      // Construct the direct download link
      Some(format!(
        "https://drive.google.com/uc?export=download&id={}",
        id
      ))
    } else {
      None
    }
  }
}

mod mediafire {
  use std::{error::Error, sync::OnceLock};

  use regex::Regex;
  use reqwest::blocking::get;

  fn extract_mediafire_id(url: &str) -> Option<&str> {
    // Define regex patterns for different MediaFire link formats
    static PATTERNS: OnceLock<[Regex; 4]> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
      [
        Regex::new(r"https://www\.mediafire\.com/file/([^/]+)/?").unwrap(),
        Regex::new(r"https://www\.mediafire\.com/view/([^/]+)/?").unwrap(),
        Regex::new(r"https://www\.mediafire\.com/download/([^/]+)/?").unwrap(),
        Regex::new(r"https://www\.mediafire\.com/?([^/]+)/?").unwrap(),
      ]
    });

    // Iterate over the patterns and try to capture the ID
    for pattern in patterns.iter() {
      if let Some(captures) = pattern.captures(url) {
        if let Some(id) = captures.get(1) {
          return Some(id.as_str());
        }
      }
    }

    // Return None if no pattern matched
    None
  }

  fn fetch_direct_download_link(url: &str) -> Result<String, Box<dyn Error>> {
    let response = get(url)?.text()?;

    let re = Regex::new(r#"https://download[0-9]+\.mediafire\.com/[^\"]+"#)?;
    if let Some(captures) = re.captures(&response) {
      if let Some(download_link) = captures.get(0) {
        return Ok(download_link.as_str().to_string());
      }
    }

    Err("Failed to find the direct download link.".into())
  }

  pub(super) fn convert_to_direct_download_link(url: &str) -> Result<String, Box<dyn Error>> {
    // Check if the URL is a valid MediaFire link
    if let Some(_) = extract_mediafire_id(url) {
      fetch_direct_download_link(url)
    } else {
      Err("Invalid MediaFire URL".into())
    }
  }
}

pub fn as_direct_download_link(url: &str) -> Option<String> {
  drive::convert_to_direct_download_link(url)
    .or_else(|| mediafire::convert_to_direct_download_link(url).ok())
}
