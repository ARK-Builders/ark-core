use std::{
    env,
    io::{Read, Seek},
    path::PathBuf,
};

use image::DynamicImage;

use pdfium_render::prelude::*;

pub enum PDFQuality {
    High,
    Medium,
    Low,
}
fn initialize_pdfium() -> Box<dyn PdfiumLibraryBindings> {
    let out_path = env!("OUT_DIR");
    let pdfium_lib_path =
        PathBuf::from(&out_path).join(Pdfium::pdfium_platform_library_name());
    let bindings = Pdfium::bind_to_library(
        #[cfg(target_os = "android")]
        Pdfium::pdfium_platform_library_name_at_path("./"),
        #[cfg(not(target_os = "android"))]
        pdfium_lib_path.to_str().unwrap(),
    )
    .or_else(|_| Pdfium::bind_to_system_library());

    match bindings {
        Ok(binding) => binding,
        Err(e) => {
            panic!("{:?}", e)
        }
    }
}
pub fn render_preview_page<R>(data: R, quailty: PDFQuality) -> DynamicImage
where
    R: Read + Seek + 'static,
{
    let render_cfg = PdfRenderConfig::new();
    let render_cfg = match quailty {
        PDFQuality::High => render_cfg.set_target_width(2000),
        PDFQuality::Medium => render_cfg,
        PDFQuality::Low => render_cfg.thumbnail(50),
    }
    .rotate_if_landscape(PdfBitmapRotation::Degrees90, true);
    Pdfium::new(initialize_pdfium())
        .load_pdf_from_reader(data, None)
        .unwrap()
        .pages()
        .get(0)
        .unwrap()
        .render_with_config(&render_cfg)
        .unwrap()
        .as_image()
}

#[test]
fn test_multi_pdf_generate() {
    use tempdir::TempDir;
    let dir = TempDir::new("arklib_test").unwrap();
    let root = dir.path();
    println!("temporary root: {}", root.display());
    for i in 0..2 {
        use std::fs::File;
        let pdf_reader = File::open("tests/test.pdf").unwrap();

        println!("Rendering {}", &i);
        let img = render_preview_page(pdf_reader, PDFQuality::High);

        img.save(root.join(format!("test{}.png", &i)))
            .expect("cannot save image");
    }
}
