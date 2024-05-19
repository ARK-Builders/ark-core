use std::path::PathBuf;

use crate::{render_preview_page, AppError, File, PDFQuality};

#[derive(Clone, Debug, clap::Args)]
#[clap(name = "render", about = "Render a PDF file to an image")]
pub struct Render {
    #[clap(value_parser, help = "The path to the file to render")]
    path: Option<PathBuf>,
    #[clap(help = "The quality of the rendering")]
    quality: Option<String>,
}

impl Render {
    pub fn run(&self) -> Result<(), AppError> {
        let filepath = self.path.to_owned().unwrap();
        let quality = match self.quality.to_owned().unwrap().as_str() {
            "high" => Ok(PDFQuality::High),
            "medium" => Ok(PDFQuality::Medium),
            "low" => Ok(PDFQuality::Low),
            _ => Err(AppError::InvalidRenderOption),
        }?;
        let buf = File::open(&filepath).unwrap();
        let dest_path = filepath.with_file_name(
            filepath
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
                + ".png",
        );
        let img = render_preview_page(buf, quality);
        img.save(dest_path).unwrap();
        Ok(())
    }
}
