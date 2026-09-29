// The booru sites: the URL of a page of results and how each site's answer becomes slides.
// Ported from frontend/js/objects/site_managers/*.js. Realbooru (an HTML scraper) is left out.
use super::slide::{iso_date, id_text, MediaType, Slide};
use regex::RegexBuilder;
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    Danbooru,
    Derpibooru,
    E621,
    Gelbooru,
    Konachan,
    Rule34,
    Safebooru,
    Tantabus,
    Xbooru,
    Yandere,
    Local,
}

impl Site {
    pub const ALL: [Site; 11] = [
        Site::Danbooru, Site::Derpibooru, Site::E621, Site::Gelbooru, Site::Konachan, Site::Rule34,
        Site::Safebooru, Site::Tantabus, Site::Xbooru, Site::Yandere, Site::Local,
    ];

    /// The id used in settings.json (sitesToSearch) and in saved favorites.
    pub fn id(self) -> &'static str {
        match self {
            Site::Danbooru => "DANB",
            Site::Derpibooru => "DERP",
            Site::E621 => "E621",
            Site::Gelbooru => "GELB",
            Site::Konachan => "KONA",
            Site::Rule34 => "RULE",
            Site::Safebooru => "SAFE",
            Site::Tantabus => "TANT",
            Site::Xbooru => "XBOO",
            Site::Yandere => "YAND",
            Site::Local => "LOCL",
        }
    }

    pub fn from_id(id: &str) -> Option<Site> {
        Site::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Site::Danbooru => "Danbooru",
            Site::Derpibooru => "Derpibooru",
            Site::E621 => "e621",
            Site::Gelbooru => "Gelbooru",
            Site::Konachan => "Konachan",
            Site::Rule34 => "Rule34",
            Site::Safebooru => "Safebooru",
            Site::Tantabus => "Tantabus",
            Site::Xbooru => "Xbooru",
            Site::Yandere => "Yande.re",
            Site::Local => "Your folders",
        }
    }

    pub fn url(self) -> &'static str {
        match self {
            Site::Danbooru => "https://danbooru.donmai.us",
            Site::Derpibooru => "https://derpibooru.org",
            Site::E621 => "https://e621.net",
            Site::Gelbooru => "https://gelbooru.com",
            Site::Konachan => "https://konachan.com",
            Site::Rule34 => "https://rule34.xxx",
            Site::Safebooru => "https://safebooru.org",
            Site::Tantabus => "https://www.tantabus.ai",
            Site::Xbooru => "https://xbooru.com",
            Site::Yandere => "https://yande.re",
            Site::Local => "",
        }
    }

    pub fn page_limit(self) -> usize {
        match self {
            Site::Derpibooru | Site::Tantabus => 50,
            _ => 100,
        }
    }
}

/// The settings a search reads, taken from settings.json when the search starts.
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub images: bool,
    pub gifs: bool,
    pub videos: bool,
    pub explicit: bool,
    pub questionable: bool,
    pub safe: bool,
    pub blacklist: HashSet<String>,
    pub derpibooru_key: String,
    pub tantabus_key: String,
    pub e621_login: String,
    pub e621_key: String,
    pub gelbooru_user: String,
    pub gelbooru_key: String,
    pub rule34_user: String,
    pub rule34_key: String,
}

impl Config {
    fn allows_media(&self, kind: MediaType) -> bool {
        match kind {
            MediaType::Image => self.images,
            MediaType::Gif => self.gifs,
            MediaType::Video => self.videos,
            MediaType::Unsupported => false,
        }
    }

    // With none of the three ticked nothing is filtered.
    fn allows_rating(&self, rating: &str) -> bool {
        if !self.explicit && !self.questionable && !self.safe {
            return true;
        }
        match rating {
            "e" => self.explicit,
            "q" => self.questionable,
            // Danbooru's "g" (general) is safe.
            "s" | "g" => self.safe,
            _ => false,
        }
    }

    pub fn is_blacklisted(&self, tags: &str) -> bool {
        tags.split_whitespace().any(|t| self.blacklist.contains(t))
    }

    fn accepts(&self, file_url: &str, rating: Option<&str>, tags: &str) -> Option<MediaType> {
        let kind = MediaType::from_path(file_url);
        (self.allows_media(kind) && rating.map_or(true, |r| self.allows_rating(r)) && !self.is_blacklisted(tags)).then_some(kind)
    }
}

/// One site's place in a search: how many pages are read and whether the results have run out.
#[derive(Clone, Debug)]
pub struct Cursor {
    pub site: Site,
    pub page: usize,
    pub exhausted: bool,
    pub pool: Option<Pool>,
}

#[derive(Clone, Debug)]
pub struct Pool {
    pub id: u64,
    pub name: String,
    pub post_ids: Vec<u64>,
}

impl Cursor {
    pub fn new(site: Site) -> Cursor {
        Cursor { site, page: 0, exhausted: false, pool: None }
    }
}

// The search words as each site wants them: the first match of each pattern is replaced, in this order.
const DANBOORU_TERMS: &[(&str, &str)] = &[
    ("sort:id", "order:id"), ("sort:id_asc", "order:id_asc"), ("sort:id_desc", "order:id_desc"),
    ("sort:score", "order:score"), ("sort:score_asc", "order:score_asc"), ("sort:score_desc", "order:score_desc"),
    ("sort:-upload", ""),
];
const PHILOMENA_TERMS: &[(&str, &str)] = &[
    ("sort:id", ""), ("sort:id_asc", ""), ("sort:id_desc", ""), ("sort:score", ""), ("sort:score_asc", ""), ("sort:score_desc", ""),
    ("order:id", ""), ("order:id_asc", ""), ("order:id_desc", ""), ("order:score", ""), ("order:score_asc", ""), ("order:score_desc", ""),
    (r"rating:s\S*", "safe"), (r"rating:q\S*", "questionable"), (r"rating:e\S*", "explicit"), ("sort:-upload", ""),
];
const GELBOORU_TERMS: &[(&str, &str)] = &[
    (r"rating:s\S*", "rating:safe"), (r"rating:q\S*", "rating:questionable"), (r"rating:e\S*", "rating:explicit"),
    ("order:score", "sort:score"), ("order:score_desc", "sort:score"), ("sort:score_desc", "sort:score"), ("sort:-upload", ""),
];
const RULE34_TERMS: &[(&str, &str)] = &[
    (r"rating:s\S*", "rating:safe"), (r"rating:q\S*", "rating:questionable"), (r"rating:e\S*", "rating:explicit"),
    ("order:id", "sort:id"), ("order:id_asc", "sort:id_asc"), ("order:id_desc", "sort:id_desc"),
    ("order:score", "sort:score"), ("order:score_asc", "sort:score_asc"), ("order:score_desc", "sort:score_desc"),
    ("sort:-upload", ""),
];

pub fn site_query(site: Site, text: &str) -> String {
    let terms = match site {
        Site::Danbooru | Site::E621 | Site::Konachan | Site::Yandere => DANBOORU_TERMS,
        Site::Derpibooru | Site::Tantabus => PHILOMENA_TERMS,
        Site::Gelbooru | Site::Xbooru => GELBOORU_TERMS,
        Site::Rule34 | Site::Safebooru => RULE34_TERMS,
        Site::Local => &[],
    };
    let mut query = text.trim().to_string();
    for (pattern, replacement) in terms {
        let re = RegexBuilder::new(pattern).case_insensitive(true).build().expect("term pattern");
        query = re.replace(&query, *replacement).into_owned();
    }
    query.trim().to_string()
}

// Philomena wants commas between the tags and spaces inside them, except in these fields.
fn philomena_query(query: &str) -> String {
    const KEEP: [&str; 7] = ["aspect_ratio", "comment_count", "created_at", "faved_by", "orig_sha512_hash", "sha512_hash", "source_url"];
    query
        .split_whitespace()
        .flat_map(|w| w.split(','))
        .map(|item| if KEEP.iter().any(|k| item.starts_with(k)) { item.to_string() } else { item.replace('_', " ") })
        .collect::<Vec<_>>()
        .join(",")
}

fn enc(s: &str) -> String {
    urlencoding::encode(s).into_owned()
}

/// An e621 pool search: "pool:123" or a pool link.
pub fn pool_id(search: &str) -> Option<u64> {
    let re = regex::Regex::new(r"(?i)^(?:pool:|https?://(?:www\.)?e621\.net/pools/)(\d+)(?:[/?#]\S*)?$").unwrap();
    re.captures(search.trim())?.get(1)?.as_str().parse().ok()
}

fn e621_login(c: &Config) -> String {
    if !c.e621_login.is_empty() && !c.e621_key.is_empty() {
        format!("&login={}&api_key={}", enc(&c.e621_login), enc(&c.e621_key))
    } else {
        String::new()
    }
}

pub fn pool_url(c: &Config, id: u64) -> String {
    format!("https://e621.net/pools/{id}.json{}", e621_login(c).replacen('&', "?", 1))
}

/// The address of the next page of results, or None when there is nothing to ask a server (own folders).
pub fn request_url(c: &Config, cursor: &Cursor, search: &str) -> Option<String> {
    let site = cursor.site;
    let page = cursor.page + 1;
    let limit = site.page_limit();
    let base = site.url();
    let query = enc(&site_query(site, search));
    Some(match site {
        Site::E621 => {
            if let Some(pool) = &cursor.pool {
                let ids: Vec<String> = pool.post_ids.iter().skip(cursor.page * limit).take(limit).map(|i| i.to_string()).collect();
                format!("{base}/posts.json?tags=id:{}&limit={limit}{}", ids.join(","), e621_login(c))
            } else {
                format!("{base}/posts.json?tags={query}&page={page}&limit={limit}{}", e621_login(c))
            }
        }
        Site::Danbooru => format!("{base}/posts.json?tags={query}&page={page}&limit={limit}"),
        Site::Konachan | Site::Yandere => format!("{base}/post.json?tags={query}&page={page}&limit={limit}"),
        Site::Derpibooru | Site::Tantabus => {
            let key = if site == Site::Derpibooru { &c.derpibooru_key } else { &c.tantabus_key };
            let key = if key.is_empty() { String::new() } else { format!("&key={}", enc(key)) };
            let q = enc(&philomena_query(&site_query(site, search)));
            format!("{base}/api/v1/json/search/images?q={q}&page={page}&per_page={limit}{key}")
        }
        Site::Gelbooru => {
            let auth = if !c.gelbooru_user.is_empty() && !c.gelbooru_key.is_empty() {
                format!("&user_id={}&api_key={}", enc(&c.gelbooru_user), enc(&c.gelbooru_key))
            } else {
                String::new()
            };
            format!("{base}/index.php?page=dapi&s=post&q=index&tags={query}&pid={}&limit={limit}{auth}", page - 1)
        }
        Site::Rule34 => {
            let auth = if !c.rule34_user.is_empty() && !c.rule34_key.is_empty() {
                format!("&user_id={}&api_key={}", enc(&c.rule34_user), enc(&c.rule34_key))
            } else {
                String::new()
            };
            format!("https://api.rule34.xxx/index.php?page=dapi&s=post&q=index{auth}&tags={query}&pid={}&limit={limit}", page - 1)
        }
        Site::Safebooru | Site::Xbooru => format!("{base}/index.php?page=dapi&s=post&q=index&tags={query}&pid={}&limit={limit}", page - 1),
        Site::Local => return None,
    })
}

/// The slides in one answer, and how many posts the answer held before filtering
/// (fewer than the page size means the results have run out).
pub fn parse(c: &Config, site: Site, body: &str) -> Result<(Vec<Slide>, usize), String> {
    match site {
        Site::E621 => json_posts(body, Some("posts")).map(|p| (p.iter().filter_map(|v| e621(c, v)).collect(), p.len())),
        Site::Danbooru => json_posts(body, None).map(|p| (p.iter().filter_map(|v| danbooru(c, v)).collect(), p.len())),
        Site::Konachan | Site::Yandere => json_posts(body, None).map(|p| (p.iter().filter_map(|v| moebooru(c, site, v)).collect(), p.len())),
        Site::Derpibooru | Site::Tantabus => json_posts(body, Some("images")).map(|p| (p.iter().filter_map(|v| philomena(c, site, v)).collect(), p.len())),
        Site::Gelbooru | Site::Rule34 | Site::Safebooru | Site::Xbooru => dapi(c, site, body),
        Site::Local => Ok((vec![], 0)),
    }
}

fn json_posts(body: &str, key: Option<&str>) -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "the answer was not JSON".to_string())?;
    let posts = match key {
        Some(k) => value.get(k).cloned(),
        None => Some(value.clone()),
    };
    match posts {
        Some(Value::Array(a)) => Ok(a),
        _ => Err(value.get("message").or_else(|| value.get("reason")).and_then(Value::as_str).unwrap_or("unexpected answer").to_string()),
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn num_of(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn absolute(base: &str, url: &str) -> String {
    if url.starts_with("//") {
        format!("https:{url}")
    } else if url.starts_with("http") {
        url.to_string()
    } else {
        format!("{base}{url}")
    }
}

fn e621(c: &Config, post: &Value) -> Option<Slide> {
    let file = post.get("file")?;
    let id = post.get("id")?.clone();
    let md5 = str_of(file, "md5").unwrap_or("").to_string();
    // Without a login a globally blacklisted post has no URL; it can be rebuilt from the md5.
    let url = match str_of(file, "url") {
        Some(u) => u.to_string(),
        None if md5.len() >= 4 && str_of(file, "ext").is_some() => {
            format!("https://static1.e621.net/data/{}/{}/{md5}.{}", &md5[..2], &md5[2..4], str_of(file, "ext")?)
        }
        None => return None,
    };
    let groups = post.get("tags")?.as_object()?.clone();
    let tags = groups.values().filter_map(Value::as_array).flatten().filter_map(Value::as_str).collect::<Vec<_>>().join(" ");
    let kind = c.accepts(&url, post.get("rating").and_then(Value::as_str), &tags)?;
    Some(Slide {
        site_id: "E621".into(),
        preview_file_url: post.get("preview").and_then(|p| str_of(p, "url")).unwrap_or(&url).to_string(),
        sample_file_url: post.get("sample").and_then(|p| str_of(p, "url")).map(String::from),
        viewable_website_post_url: format!("https://e621.net/posts/{}", id_text(&id)),
        width: num_of(file, "width"),
        height: num_of(file, "height"),
        date: iso_date(str_of(post, "created_at").unwrap_or("")),
        score: post.get("score").map(|s| num_of(s, "total")).filter(|n| *n != 0).unwrap_or_else(|| num_of(post, "score")),
        media_type: kind,
        md5,
        tags,
        tag_groups: Some(groups),
        file_url: url,
        id,
    })
}

fn danbooru(c: &Config, post: &Value) -> Option<Slide> {
    let base = Site::Danbooru.url();
    let file = absolute(base, str_of(post, "file_url")?);
    let preview = absolute(base, str_of(post, "preview_file_url").unwrap_or(&file));
    let tags = str_of(post, "tag_string").unwrap_or("").to_string();
    let kind = c.accepts(&file, post.get("rating").and_then(Value::as_str), &tags)?;
    let mut groups = Map::new();
    for category in ["artist", "copyright", "character", "general", "meta"] {
        let words = str_of(post, &format!("tag_string_{category}")).unwrap_or("").split(' ').filter(|w| !w.is_empty()).map(|w| Value::String(w.into())).collect();
        groups.insert(category.into(), Value::Array(words));
    }
    let id = post.get("id")?.clone();
    Some(Slide {
        site_id: "DANB".into(),
        viewable_website_post_url: format!("{base}/posts/{}", id_text(&id)),
        preview_file_url: preview,
        sample_file_url: str_of(post, "large_file_url").map(|u| absolute(base, u)),
        width: num_of(post, "image_width"),
        height: num_of(post, "image_height"),
        date: iso_date(str_of(post, "created_at").unwrap_or("")),
        score: num_of(post, "score"),
        media_type: kind,
        md5: str_of(post, "md5").unwrap_or("").into(),
        tags,
        tag_groups: Some(groups),
        file_url: file,
        id,
    })
}

fn moebooru(c: &Config, site: Site, post: &Value) -> Option<Slide> {
    let file = str_of(post, "file_url")?.to_string();
    let tags = str_of(post, "tags").unwrap_or("").to_string();
    let kind = c.accepts(&file, post.get("rating").and_then(Value::as_str), &tags)?;
    let id = post.get("id")?.clone();
    let created = post.get("created_at");
    let seconds = created.and_then(|v| v.get("s")).or(created).map(|v| id_text(v)).unwrap_or_default();
    Some(Slide {
        site_id: site.id().into(),
        viewable_website_post_url: format!("{}/post/show/{}", site.url(), id_text(&id)),
        preview_file_url: str_of(post, "preview_url").unwrap_or(&file).into(),
        width: num_of(post, "width"),
        height: num_of(post, "height"),
        date: iso_date(&seconds),
        score: num_of(post, "score"),
        media_type: kind,
        md5: str_of(post, "md5").unwrap_or("").into(),
        tags,
        file_url: file,
        id,
        ..Default::default()
    })
}

fn philomena(c: &Config, site: Site, post: &Value) -> Option<Slide> {
    let reps = post.get("representations")?;
    let full = str_of(reps, "full")?.to_string();
    let tag_list: Vec<&str> = post.get("tags")?.as_array()?.iter().filter_map(Value::as_str).collect();
    let tags = tag_list.iter().map(|t| t.replace(' ', "_")).collect::<Vec<_>>().join(" ");
    let has = |t: &str| tag_list.contains(&t);
    let rating = if has("explicit") { "e" } else if has("suggestive") || has("questionable") { "q" } else { "s" };
    let kind = c.accepts(&full, Some(rating), &tags)?;
    let id = post.get("id")?.clone();
    Some(Slide {
        site_id: site.id().into(),
        viewable_website_post_url: format!("{}/images/{}", site.url(), id_text(&id)),
        preview_file_url: str_of(reps, "thumb").unwrap_or(&full).into(),
        width: num_of(post, "width"),
        height: num_of(post, "height"),
        date: iso_date(str_of(post, "created_at").unwrap_or("")),
        score: num_of(post, "score"),
        media_type: kind,
        md5: str_of(post, "sha512_hash").unwrap_or("").into(),
        tags,
        file_url: full,
        id,
        ..Default::default()
    })
}

// Gelbooru writes each field as an element, the others as attributes.
fn dapi(c: &Config, site: Site, body: &str) -> Result<(Vec<Slide>, usize), String> {
    let doc = roxmltree::Document::parse(body).map_err(|_| "the answer was not XML".to_string())?;
    if let Some(e) = doc.descendants().find(|n| n.has_tag_name("error")) {
        let text = e.text().unwrap_or("").trim().trim_end_matches('.').to_string();
        return Err(match site {
            Site::Rule34 => format!("{text}. Add your Rule34 user ID and API key in Settings → Sites & accounts."),
            _ => text,
        });
    }
    let posts: Vec<_> = doc.descendants().filter(|n| n.has_tag_name("post")).collect();
    let slides = posts
        .iter()
        .filter_map(|p| {
            let field = |name: &str| -> String {
                p.attribute(name).map(String::from).or_else(|| p.children().find(|n| n.has_tag_name(name)).and_then(|n| n.text()).map(String::from)).unwrap_or_default()
            };
            let (file, preview) = (field("file_url"), field("preview_url"));
            if file.is_empty() {
                return None;
            }
            let tags = field("tags");
            let tags = tags.trim().to_string();
            // Safebooru and Gelbooru had no rating filter in the web version; Safebooru is all safe anyway.
            let rating = (!matches!(site, Site::Safebooru | Site::Gelbooru)).then(|| field("rating"));
            let kind = c.accepts(&file, rating.as_deref(), &tags)?;
            let id = field("id");
            let fix = |u: &str| if let Some(rest) = u.strip_prefix("//") { format!("https://{rest}") } else { u.to_string() };
            let view = format!("{}/index.php?page=post&s=view&id={id}", site.url());
            Some(Slide {
                site_id: site.id().into(),
                preview_file_url: if site == Site::Safebooru || preview.is_empty() { fix(&file) } else { fix(&preview) },
                viewable_website_post_url: view,
                width: field("width").parse().unwrap_or(0),
                height: field("height").parse().unwrap_or(0),
                date: iso_date(&field("created_at")),
                score: field("score").parse().unwrap_or(0),
                media_type: kind,
                md5: field("md5"),
                tags,
                file_url: fix(&file),
                id: Value::String(id),
                ..Default::default()
            })
        })
        .collect();
    Ok((slides, posts.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config { images: true, gifs: true, videos: true, blacklist: ["bad".to_string()].into(), ..Default::default() }
    }

    #[test]
    fn query_terms_per_site() {
        assert_eq!(site_query(Site::E621, "cat sort:score"), "cat order:score");
        assert_eq!(site_query(Site::Rule34, "cat rating:safe order:score_desc"), "cat rating:safe sort:score_desc");
        assert_eq!(site_query(Site::Derpibooru, "pony rating:s sort:score"), "pony safe");
        assert_eq!(site_query(Site::Gelbooru, "x rating:q"), "x rating:questionable");
    }

    #[test]
    fn philomena_query_form() {
        assert_eq!(philomena_query("rainbow_dash safe aspect_ratio:1.0"), "rainbow dash,safe,aspect_ratio:1.0");
    }

    #[test]
    fn pool_links() {
        assert_eq!(pool_id("pool:17870"), Some(17870));
        assert_eq!(pool_id("https://e621.net/pools/5?x=1"), Some(5));
        assert_eq!(pool_id("cat pool:1"), None);
    }

    #[test]
    fn e621_posts() {
        let body = r#"{"posts":[
          {"id":1,"created_at":"2024-01-01T12:00:00.000+00:00","score":{"total":7},"rating":"s","file":{"url":"https://static1.e621.net/data/aa/bb/aabb.jpg","md5":"aabb","width":10,"height":20,"ext":"jpg"},"preview":{"url":"https://p/x.jpg"},"sample":{"url":null},"tags":{"general":["cat","bad"],"artist":["me"]}},
          {"id":2,"created_at":"2024-01-01T12:00:00.000+00:00","score":{"total":1},"rating":"e","file":{"url":null,"md5":"ccddee","ext":"webm"},"preview":{"url":null},"tags":{"general":["dog"]}},
          {"id":3,"created_at":"2024-01-01T12:00:00.000+00:00","score":{"total":1},"rating":"e","file":{"url":null,"md5":"ccddee","ext":"webm"},"preview":{"url":null},"tags":{"general":["dog"]}}]}"#;
        let (slides, n) = parse(&config(), Site::E621, body).unwrap();
        assert_eq!(n, 3);
        // Post 1 has a blacklisted tag; 2 and 3 get their URL from the md5.
        assert_eq!(slides.len(), 2);
        assert_eq!(slides[0].file_url, "https://static1.e621.net/data/cc/dd/ccddee.webm");
        assert_eq!(slides[0].media_type, MediaType::Video);
        let only_safe = Config { safe: true, ..config() };
        assert_eq!(parse(&only_safe, Site::E621, body).unwrap().0.len(), 0);
    }

    #[test]
    fn danbooru_general_counts_as_safe() {
        let body = r#"[{"id":5,"rating":"g","file_url":"/data/a.png","preview_file_url":"/p.jpg","tag_string":"a b","tag_string_artist":"x","md5":"m","score":3,"image_width":1,"image_height":2,"created_at":"2024-01-01T00:00:00.000-05:00"}]"#;
        let c = Config { safe: true, ..config() };
        let (slides, _) = parse(&c, Site::Danbooru, body).unwrap();
        assert_eq!(slides[0].file_url, "https://danbooru.donmai.us/data/a.png");
        assert_eq!(slides[0].tag_groups.as_ref().unwrap()["artist"][0], "x");
    }

    #[test]
    fn philomena_rating_from_tags() {
        let body = r#"{"images":[{"id":9,"representations":{"full":"https://f/a.png","thumb":"https://f/t.png"},"tags":["safe","rainbow dash"],"width":1,"height":1,"score":2,"created_at":"2024-01-01T00:00:00Z","sha512_hash":"h"}]}"#;
        let (slides, _) = parse(&Config { explicit: true, ..config() }, Site::Derpibooru, body).unwrap();
        assert!(slides.is_empty());
        let (slides, _) = parse(&config(), Site::Derpibooru, body).unwrap();
        assert_eq!(slides[0].tags, "safe rainbow_dash");
    }

    #[test]
    fn xml_attributes_and_elements() {
        let attrs = r#"<posts><post id="1" file_url="//img/a.jpg" preview_url="//img/t.jpg" tags=" a b " rating="s" width="3" height="4" score="5" md5="m" created_at="Mon Jan 01 12:00:00 +0000 2024"/></posts>"#;
        let (slides, n) = parse(&config(), Site::Xbooru, attrs).unwrap();
        assert_eq!((n, slides[0].file_url.as_str(), slides[0].tags.as_str()), (1, "https://img/a.jpg", "a b"));
        let elements = "<posts><post><id>2</id><file_url>https://i/b.gif</file_url><preview_url>https://i/t.jpg</preview_url><tags>x</tags><md5>q</md5></post></posts>";
        assert_eq!(parse(&config(), Site::Gelbooru, elements).unwrap().0[0].media_type, MediaType::Gif);
        let error = "<response><error>Missing authentication.</error></response>";
        assert!(parse(&config(), Site::Rule34, error).unwrap_err().starts_with("Missing authentication."));
    }
}
