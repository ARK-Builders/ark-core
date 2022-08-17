use anyhow::Error;
use reqwest::header::HeaderValue;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::{collections::hash_map::DefaultHasher, fmt};
use std::{fs::File, io::Write, path::PathBuf};
use url::Url;

/// .link File used in ARK Shelf.
#[derive(Debug, Deserialize, Serialize)]
pub struct Link {
    title: String,
    desc: String,
    url: Url,
}

impl Link {
    pub fn new(title: String, desc: String, url: Url) -> Self {
        Self { title, desc, url }
    }
    /// Get formatted name for .link
    pub fn format_name(&self) -> String {
        let mut s = DefaultHasher::new();

        let url = self
            .url
            .to_string()
            .replace("http://", "")
            .replace("https://", "")
            .split(&['-', '?', '/'][..])
            .filter(|x| x != &"")
            .collect::<Vec<&str>>()
            .join("-");
        url.hash(&mut s);
        s.finish().to_string()
    }

    // Load the link json from the .link file.
    pub fn load_json<P: AsRef<Path>>(path: P) -> Result<String, Error> {
        let p = path.as_ref().to_path_buf();
        let link = Link::from(p);

        let json = serde_json::to_string(&link).unwrap();
        Ok(json)
    }

    // Load the image.png file from the .link file if exists.
    pub fn load_preview<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, Error> {
        let file = File::open(path.as_ref()).expect("Open link file");
        let mut zip = zip::ZipArchive::new(file.try_clone().unwrap())
            .expect("Open zip archive");
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).unwrap();
            let path = file.enclosed_name().unwrap();
            if path.to_str() == Some("link.png") {
                let mut dst: Vec<u8> = Vec::new();
                std::io::copy(&mut file, &mut dst).unwrap();
                return Ok(dst);
            }
        }
        Err(Error::msg("An image.png file not found in the zip file"))
    }

    /// Write zipped file to path
    pub async fn write_to_path<P: AsRef<Path>>(
        &mut self,
        path: P,
        download_preview: bool,
    ) {
        let j = serde_json::to_string(self).unwrap();
        let link_file = File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(link_file);
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("link.json", options)
            .expect("cannot create link.json");
        zip.write(j.as_bytes()).unwrap();
        if download_preview {
            let preview_data = Link::get_preview(self.url.clone())
                .await
                .unwrap_or_default();
            let image_data = preview_data
                .fetch_image()
                .await
                .unwrap_or_default();
            zip.start_file("link.png", options).unwrap();
            zip.write(&image_data).unwrap();
        }
        zip.finish().unwrap();
    }

    /// Synchronized version of Write zipped file to path
    pub fn write_to_path_sync<P: AsRef<Path>>(
        &mut self,
        path: P,
        download_preview: bool,
    ) {
        let runtime =
            tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        runtime.block_on(self.write_to_path(path, download_preview));
    }
    /// Get metadata of the link.
    pub async fn get_preview<S>(url: S) -> Result<OpenGraph, reqwest::Error>
    where
        S: Into<String>,
    {
        let mut header = reqwest::header::HeaderMap::new();
        header.insert(
            "User-Agent",
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64; rv:102.0) Gecko/20100101 Firefox/102.0",
            ),
        );
        let client = reqwest::Client::builder()
            .default_headers(header)
            .build()
            .unwrap();
        let scraper = client
            .get(url.into())
            .send()
            .await?
            .text()
            .await?;
        let html = Html::parse_document(&scraper.as_str());
        let title =
            select_og(&html, OpenGraphTag::Title).or(select_title(&html));
        Ok(OpenGraph {
            title,
            description: select_og(&html, OpenGraphTag::Description)
                .or(select_desc(&html)),
            url: select_og(&html, OpenGraphTag::Url),
            image: select_og(&html, OpenGraphTag::Image),
            object_type: select_og(&html, OpenGraphTag::Type),
            locale: select_og(&html, OpenGraphTag::Locale),
        })
    }
}

fn select_og(html: &Html, tag: OpenGraphTag) -> Option<String> {
    let selector =
        Selector::parse(&format!("meta[property=\"og:{}\"]", tag.as_str()))
            .unwrap();

    if let Some(element) = html.select(&selector).next() {
        if let Some(value) = element.value().attr("content") {
            return Some(value.to_string());
        }
    }

    None
}
fn select_desc(html: &Html) -> Option<String> {
    let selector = Selector::parse("meta[name=\"description\"]").unwrap();

    if let Some(element) = html.select(&selector).next() {
        if let Some(value) = element.value().attr("content") {
            return Some(value.to_string());
        }
    }

    None
}
fn select_title(html: &Html) -> Option<String> {
    let selector = Selector::parse("title").unwrap();
    if let Some(element) = html.select(&selector).next() {
        return element.text().next().map(|x| x.to_string());
    }

    None
}
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct OpenGraph {
    /// Represents the "og:title" OpenGraph meta tag.
    ///
    /// The title of your object as it should appear within
    /// the graph, e.g., "The Rock".
    title: Option<String>,
    /// Represents the "og:description" OpenGraph meta tag
    description: Option<String>,
    /// Represents the "og:url" OpenGraph meta tag
    url: Option<String>,
    /// Represents the "og:image" OpenGraph meta tag
    image: Option<String>,
    /// Represents the "og:type" OpenGraph meta tag
    ///
    /// The type of your object, e.g., "video.movie". Depending on the type
    /// you specify, other properties may also be required.
    object_type: Option<String>,
    /// Represents the "og:locale" OpenGraph meta tag
    locale: Option<String>,
}
impl OpenGraph {
    pub async fn fetch_image(&self) -> Option<Vec<u8>> {
        if let Some(url) = &self.image {
            let res = reqwest::get(url).await.unwrap();
            Some(res.bytes().await.unwrap().to_vec())
        } else {
            None
        }
    }
}
/// OpenGraphTag meta tags collection
pub enum OpenGraphTag {
    /// Represents the "og:title" OpenGraph meta tag.
    ///
    /// The title of your object as it should appear within
    /// the graph, e.g., "The Rock".
    Title,
    /// Represents the "og:url" OpenGraph meta tag
    Url,
    /// Represents the "og:image" OpenGraph meta tag
    Image,
    /// Represents the "og:type" OpenGraph meta tag
    ///
    /// The type of your object, e.g., "video.movie". Depending on the type
    /// you specify, other properties may also be required.
    Type,
    /// Represents the "og:description" OpenGraph meta tag
    Description,
    /// Represents the "og:locale" OpenGraph meta tag
    Locale,
    /// Represents the "og:image:height" OpenGraph meta tag
    ImageHeight,
    /// Represents the "og:image:width" OpenGraph meta tag
    ImageWidth,
    /// Represents the "og:site_name" OpenGraph meta tag
    SiteName,
}

impl fmt::Debug for OpenGraphTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl OpenGraphTag {
    fn as_str(&self) -> &str {
        match self {
            OpenGraphTag::Title => "title",
            OpenGraphTag::Url => "url",
            OpenGraphTag::Image => "image",
            OpenGraphTag::Type => "type",
            OpenGraphTag::Description => "description",
            OpenGraphTag::Locale => "locale",
            OpenGraphTag::ImageHeight => "image:height",
            OpenGraphTag::ImageWidth => "image:width",
            OpenGraphTag::SiteName => "site_name",
        }
    }
}

impl From<PathBuf> for Link {
    fn from(path: PathBuf) -> Self {
        let file = File::open(path).expect("Open link file");
        let mut zip = zip::ZipArchive::new(file.try_clone().unwrap())
            .expect("Open zip archive");
        let j_raw = zip
            .by_name("link.json")
            .expect("Find link.json in the zip archive");

        let j = serde_json::from_reader(j_raw).expect("Parse link.json");
        Self { ..j }
    }
}

#[test]
fn test_create_link_file() {
    use tempdir::TempDir;
    let dir = TempDir::new("arklib_test").unwrap();
    let tmp_path = dir.path();
    println!("temp path: {}", tmp_path.display());
    let url = Url::parse("https://example.com/").unwrap();
    let mut link = Link::new(String::from("title"), String::from("desc"), url);
    let hash = link.format_name();
    assert_eq!(hash, "5257664237369877164");
    let link_file_path = tmp_path.join(format!("{}.link", hash));
    for download_preview in [true, false] {
        link.write_to_path_sync(link_file_path.clone(), download_preview);
        let link_json = Link::load_json(link_file_path.clone()).unwrap();
        let j: Link = serde_json::from_str(link_json.as_str()).unwrap();
        assert_eq!(j.title, "title");
        assert_eq!(j.desc, "desc");
        assert_eq!(j.url.as_str(), "https://example.com/");
        let _ = match Link::load_preview(link_file_path.clone()) {
            Ok(_) => assert_eq!(download_preview, true),
            Err(_) => assert_eq!(download_preview, false),
        };
    }
}
