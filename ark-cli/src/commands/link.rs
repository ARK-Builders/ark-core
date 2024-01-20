use std::path::PathBuf;

use arklib::{id::ResourceId, link::Link};
use url::Url;

use crate::provide_index;

pub async fn create_link(
    root: &PathBuf,
    url: &str,
    title: &str,
    desc: Option<String>,
) -> Result<(), String> {
    let url = Url::parse(url).map_err(|_| "Invalid url")?;
    let link: Link = Link::new(url, title.to_owned(), desc.to_owned());
    link.save(&root, true)
        .await
        .map_err(|e| e.to_string())
}

pub fn load_link(
    root: &PathBuf,
    file_path: &Option<PathBuf>,
    id: &Option<ResourceId>,
) -> Result<Link, String> {
    let path_from_index = id.map(|id| {
        let index = provide_index(&root);
        index.id2path[&id].as_path().to_path_buf()
    });
    let path_from_user = file_path;

    let path = match (path_from_user, path_from_index) {
        (Some(path), Some(path2)) => {
            if path.canonicalize().unwrap() != path2 {
                Err(format!(
                    "Path {:?} was requested. But id {} maps to path {:?}",
                    path,
                    id.unwrap(),
                    path2,
                ))
            } else {
                Ok(path.to_path_buf())
            }
        }
        (Some(path), None) => Ok(path.to_path_buf()),
        (None, Some(path)) => Ok(path),
        (None, None) => Err("Provide a path or id for request.".to_owned())?,
    }?;

    arklib::link::Link::load(root, &path).map_err(|e| e.to_string())
}
