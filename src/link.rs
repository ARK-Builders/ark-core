use anyhow::Error;
use reqwest::header::HeaderValue;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::str::{self, FromStr};
use std::{collections::hash_map::DefaultHasher, fmt};
use std::{fs, fs::File, io::Write, path::PathBuf};
use url::Url;

use crate::id::ResourceId;
use crate::meta2::{load_meta_bytes, store_meta};

const PREVIEWS_RELATIVE_PATH: &str = ".ark/previews";

#[derive(Debug, Deserialize, Serialize)]
pub struct Link {
    pub url: Url,
    pub meta: Metadata,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Metadata {
    pub title: String,
    pub desc: Option<String>,
}

impl Link {
    pub fn new(url: Url, title: String, desc: Option<String>) -> Self {
        Self {
            url: url,
            meta: Metadata { title, desc },
        }
    }

    pub fn id(&self) -> ResourceId {
        ResourceId::compute_bytes(self.url.as_str().as_bytes())
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

    /// Load a link with its metadata from file
    pub fn load<P: AsRef<Path>>(root: P, path: P) -> Result<Self, Error> {
        let p = path.as_ref().to_path_buf();
        let url = Self::load_url(p)?;
        let id = ResourceId::compute_bytes(url.as_str().as_bytes());

        let bytes = load_meta_bytes::<PathBuf>(root.as_ref().to_owned(), id)?;
        let meta: Metadata = serde_json::from_slice(&bytes)?;

        Ok(Self { url, meta })
    }

    /// Write zipped file to path
    pub async fn write_to_path<P: AsRef<Path>>(
        &mut self,
        root: P,
        path: P,
        save_preview: bool,
    ) {
        let id = self.id();
        store_meta::<Metadata, _>(root.as_ref(), id, &self.meta);

        let mut link_file = File::create(path.as_ref().to_owned()).unwrap();
        let file_data = self.url.as_str().as_bytes();
        link_file.write(file_data).unwrap();
        if save_preview {
            let preview_data = Link::get_preview(self.url.clone())
                .await
                .unwrap_or_default();
            let image_data = preview_data
                .fetch_image()
                .await
                .unwrap_or_default();
            self.save_preview(root.as_ref(), image_data, id, file_data.len())
                .await;
        }

        store_meta::<Metadata, _>(root, id, &self.meta);
    }

    /// Synchronized version of Write zipped file to path
    pub fn write_to_path_sync<P: AsRef<Path>>(
        &mut self,
        root: P,
        path: P,
        save_preview: bool,
    ) {
        let runtime =
            tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        runtime.block_on(self.write_to_path(root, path, save_preview));
    }

    pub async fn save_preview<P: AsRef<Path>>(
        &mut self,
        root: P,
        image_data: Vec<u8>,
        resource_id: ResourceId,
        data_size: usize,
    ) {
        let previews_path = root.as_ref().join(PREVIEWS_RELATIVE_PATH);
        fs::create_dir_all(previews_path.to_owned())
            .expect(&format!("Creating {} directory", PREVIEWS_RELATIVE_PATH));
        let mut preview_file = File::create(
            previews_path
                .to_owned()
                .join(format!("{}-{}.png", data_size, resource_id.crc32)),
        )
        .unwrap();
        preview_file.write(image_data.as_slice()).unwrap();
    }

    /// Get metadata of the link (synced).
    pub fn get_preview_synced<S>(url: S) -> Result<OpenGraph, reqwest::Error>
    where
        S: Into<String>,
    {
        let runtime =
            tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        return runtime.block_on(Link::get_preview(url));
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

    fn load_url(path: PathBuf) -> Result<Url, Error> {
        let url_raw = std::fs::read(path)?;
        let url_str = str::from_utf8(url_raw.as_slice()).unwrap();
        Ok(Url::from_str(url_str)?)
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
    pub title: Option<String>,
    /// Represents the "og:description" OpenGraph meta tag
    pub description: Option<String>,
    /// Represents the "og:url" OpenGraph meta tag
    pub url: Option<String>,
    /// Represents the "og:image" OpenGraph meta tag
    pub image: Option<String>,
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

#[test]
fn test_create_link_file() {
    use tempdir::TempDir;
    let dir = TempDir::new("arklib_test").unwrap();
    let root = dir.path();
    println!("temporary root: {}", root.display());
    let url = Url::parse("https://example.com/").unwrap();
    let mut link =
        Link::new(url, String::from("title"), Some(String::from("desc")));

    let hash = link.format_name();
    assert_eq!(hash, "5257664237369877164");
    let link_file_path = root.join(format!("{}.link", hash));
    for save_preview in [false, true] {
        link.write_to_path_sync(root, link_file_path.as_path(), save_preview);
        let link_file_bytes = std::fs::read(link_file_path.to_owned()).unwrap();
        let url: Url =
            Url::from_str(str::from_utf8(&link_file_bytes).unwrap()).unwrap();
        assert_eq!(url.as_str(), "https://example.com/");
        let link = Link::load(root.clone(), link_file_path.as_path()).unwrap();
        assert_eq!(link.url.as_str(), url.as_str());
        assert_eq!(link.meta.desc.unwrap(), "desc");
        assert_eq!(link.meta.title, "title");

        let resource_id = ResourceId::compute_bytes(link_file_bytes.as_slice());
        println!("resource: {}, {}", resource_id.crc32, resource_id.data_size);

        if Path::new(root)
            .join(PREVIEWS_RELATIVE_PATH)
            .join(format!(
                "{}-{}.png",
                link_file_bytes.len(),
                resource_id.crc32
            ))
            .exists()
        {
            assert_eq!(save_preview, true)
        } else {
            assert_eq!(save_preview, false)
        }
    }
}
