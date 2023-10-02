use reqwest::header::HeaderValue;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::{self, FromStr};
use std::{fs, fs::File, io::Write, path::PathBuf};
use url::Url;

use crate::id::ResourceId;
use crate::meta::{load_meta_bytes, store_meta};
use crate::{ArklibError, Result, ARK_FOLDER, PREVIEWS_STORAGE_FOLDER};

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
            url,
            meta: Metadata { title, desc },
        }
    }

    pub fn id(&self) -> Result<ResourceId> {
        ResourceId::compute_bytes(self.url.as_str().as_bytes())
    }

    /// Load a link with its metadata from file
    pub fn load<P: AsRef<Path>>(root: P, path: P) -> Result<Self> {
        let p = path.as_ref().to_path_buf();
        let url = Self::load_url(p)?;
        let id = ResourceId::compute_bytes(url.as_str().as_bytes())?;

        let bytes = load_meta_bytes::<PathBuf>(root.as_ref().to_owned(), id)?;
        let meta: Metadata =
            serde_json::from_slice(&bytes).map_err(|_| ArklibError::Parse)?;

        Ok(Self { url, meta })
    }

    /// Write zipped file to path
    pub async fn write_to_path<P: AsRef<Path>>(
        &mut self,
        root: P,
        path: P,
        save_preview: bool,
    ) -> Result<()> {
        let id = self.id()?;
        store_meta::<Metadata, _>(root.as_ref(), id, &self.meta)?;

        let mut link_file = File::create(path.as_ref().to_owned())?;
        let file_data = self.url.as_str().as_bytes();
        link_file.write(file_data)?;
        if save_preview {
            let preview_data = Link::get_preview(self.url.clone())
                .await
                .unwrap_or_default();
            let image_data = preview_data
                .fetch_image()
                .await
                .unwrap_or_default();
            self.save_preview(root.as_ref(), image_data, id)
                .await?;
        }

        store_meta::<Metadata, _>(root, id, &self.meta)
    }

    /// Synchronized version of Write zipped file to path
    pub fn write_to_path_sync<P: AsRef<Path>>(
        &mut self,
        root: P,
        path: P,
        save_preview: bool,
    ) -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.write_to_path(root, path, save_preview))
    }

    pub async fn save_preview<P: AsRef<Path>>(
        &mut self,
        root: P,
        image_data: Vec<u8>,
        id: ResourceId,
    ) -> Result<()> {
        let path = root
            .as_ref()
            .join(ARK_FOLDER)
            .join(PREVIEWS_STORAGE_FOLDER);
        fs::create_dir_all(path.to_owned())?;

        let file = path.to_owned().join(id.to_string());

        let mut file = File::create(file)?;
        file.write(image_data.as_slice())?;

        Ok(())
    }

    /// Get metadata of the link (synced).
    pub fn get_preview_synced<S>(url: S) -> Result<OpenGraph>
    where
        S: Into<String>,
    {
        let runtime =
            tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        return runtime.block_on(Link::get_preview(url));
    }

    /// Get metadata of the link.
    pub async fn get_preview<S>(url: S) -> Result<OpenGraph>
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
            .build()?;
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

    fn load_url(path: PathBuf) -> Result<Url> {
        let url_raw = std::fs::read(path)?;
        let url_str = str::from_utf8(url_raw.as_slice())?;
        Url::from_str(url_str).map_err(|_| ArklibError::Parse)
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

    pub fn fetch_image_synced(&self) -> Option<Vec<u8>> {
        let runtime =
            tokio::runtime::Runtime::new().expect("Unable to create a runtime");
        return runtime.block_on(self.fetch_image());
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

    let path = root.join("test.link");

    for save_preview in [false, true] {
        link.write_to_path_sync(root, path.as_path(), save_preview)
            .unwrap();
        let link_file_bytes = std::fs::read(path.to_owned()).unwrap();
        let url: Url =
            Url::from_str(str::from_utf8(&link_file_bytes).unwrap()).unwrap();
        assert_eq!(url.as_str(), "https://example.com/");
        let link = Link::load(root.clone(), path.as_path()).unwrap();
        assert_eq!(link.url.as_str(), url.as_str());
        assert_eq!(link.meta.desc.unwrap(), "desc");
        assert_eq!(link.meta.title, "title");

        let id = ResourceId::compute_bytes(link_file_bytes.as_slice()).unwrap();
        println!("resource: {}, {}", id.crc32, id.data_size);

        if Path::new(root)
            .join(ARK_FOLDER)
            .join(PREVIEWS_STORAGE_FOLDER)
            .join(id.to_string())
            .exists()
        {
            assert_eq!(save_preview, true)
        } else {
            assert_eq!(save_preview, false)
        }
    }
}
