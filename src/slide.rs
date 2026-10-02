// One post, in the shape the favorites list has always saved it (settings.json → personalListItems),
// so favorites made in the web version load here and the other way round.
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Slide {
    pub site_id: String,
    // A number on some sites and a string on others; kept as it was so a saved list stays the same.
    pub id: Value,
    pub file_url: String,
    pub preview_file_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_file_url: Option<String>,
    pub viewable_website_post_url: String,
    #[serde(deserialize_with = "number")]
    pub width: i64,
    #[serde(deserialize_with = "number")]
    pub height: i64,
    // ISO 8601, as JSON.stringify writes a JS Date.
    pub date: String,
    #[serde(deserialize_with = "number")]
    pub score: i64,
    pub media_type: MediaType,
    #[serde(deserialize_with = "text")]
    pub md5: String,
    pub tags: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_groups: Option<Map<String, Value>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MediaType {
    #[default]
    Image,
    Gif,
    Video,
    Unsupported,
}

impl MediaType {
    pub fn from_path(path: &str) -> MediaType {
        let path = path.split(['?', '#']).next().unwrap_or("").to_lowercase();
        let ext = path.rsplit('.').next().unwrap_or("");
        match ext {
            "webm" | "mp4" | "m4v" | "mov" | "ogv" | "mkv" => MediaType::Video,
            "gif" => MediaType::Gif,
            "swf" | "zip" => MediaType::Unsupported,
            _ => MediaType::Image,
        }
    }
}

impl Slide {
    pub fn key(&self) -> (String, String) {
        (self.site_id.clone(), id_text(&self.id))
    }

    /// Seconds since 1970, for sorting; 0 when the date can't be read.
    pub fn timestamp(&self) -> i64 {
        parse_date(&self.date)
    }
}

/// (category, tags): by category in e621's order when the site gave them, else one list.
pub fn tag_groups(slide: &Slide) -> Vec<(String, Vec<String>)> {
    const ORDER: [&str; 9] = ["artist", "copyright", "character", "species", "general", "meta", "lore", "invalid", "contributor"];
    match &slide.tag_groups {
        Some(groups) if groups.values().any(|v| v.as_array().is_some_and(|a| !a.is_empty())) => {
            let mut names: Vec<&String> = groups.keys().collect();
            names.sort_by_key(|n| ORDER.iter().position(|o| o == n).unwrap_or(99));
            names
                .into_iter()
                .filter_map(|n| {
                    let tags: Vec<String> = groups[n].as_array()?.iter().filter_map(|t| t.as_str().map(String::from)).collect();
                    (!tags.is_empty()).then(|| (n.clone(), tags))
                })
                .collect()
        }
        _ => vec![(String::new(), slide.tags.split_whitespace().map(String::from).collect())],
    }
}

pub fn id_text(id: &Value) -> String {
    match id {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The ISO string a JS Date would be saved as. Sites write dates in a few forms (e621: ISO, Gelbooru:
/// "Mon Jan 01 12:00:00 +0000 2024", Moebooru: Unix seconds).
pub fn iso_date(text: &str) -> String {
    use chrono::{DateTime, TimeZone, Utc};
    let ts = parse_date(text);
    Utc.timestamp_opt(ts, 0).single().map(|d: DateTime<Utc>| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)).unwrap_or_default()
}

fn parse_date(text: &str) -> i64 {
    use chrono::DateTime;
    let text = text.trim();
    if let Ok(n) = text.parse::<i64>() {
        return n;
    }
    DateTime::parse_from_rfc3339(text)
        .or_else(|_| DateTime::parse_from_str(text, "%a %b %d %H:%M:%S %z %Y"))
        .or_else(|_| DateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S %z"))
        .or_else(|_| DateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f%z"))
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

fn number<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

// Local files have the address in md5; sites sometimes leave it out (null).
fn text<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        Value::Null => String::new(),
        other => other.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_from_url() {
        assert_eq!(MediaType::from_path("https://x/a.WEBM?x=1"), MediaType::Video);
        assert_eq!(MediaType::from_path("a.gif"), MediaType::Gif);
        assert_eq!(MediaType::from_path("a.swf"), MediaType::Unsupported);
        assert_eq!(MediaType::from_path("a.jpeg"), MediaType::Image);
    }

    #[test]
    fn dates() {
        assert_eq!(iso_date("Mon Jan 01 12:00:00 +0000 2024"), "2024-01-01T12:00:00.000Z");
        assert_eq!(iso_date("1704110400"), "2024-01-01T12:00:00.000Z");
        assert_eq!(iso_date("2024-01-01T12:00:00.000+00:00"), "2024-01-01T12:00:00.000Z");
    }

    // A favorite saved by the web version, with mixed types, loads and saves back with the same id.
    #[test]
    fn saved_favorite_round_trips() {
        let json = r#"{"siteId":"E621","id":123,"fileUrl":"u","previewFileUrl":"p","viewableWebsitePostUrl":"v","width":"10","height":20,"date":"2024-01-01T00:00:00.000Z","score":5,"mediaType":"VIDEO","md5":null,"tags":"a b","tagGroups":{"artist":["x"]}}"#;
        let slide: Slide = serde_json::from_str(json).unwrap();
        assert_eq!((slide.width, slide.media_type), (10, MediaType::Video));
        let back = serde_json::to_value(&slide).unwrap();
        assert_eq!(back["id"], 123);
        assert_eq!(back["tagGroups"]["artist"][0], "x");
    }
}
