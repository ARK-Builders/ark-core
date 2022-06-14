use std::{env, ffi::OsString, path::PathBuf};

use image::DynamicImage;

use pdfium_render::prelude::*;

pub enum PDFQuailty {
    High,
    Medium,
    Low,
}
fn initialize_pdfium() -> Box<dyn PdfiumLibraryBindings> {
    let out_path = env!("OUT_DIR");
    
    let pdfium_libpath =
        PathBuf::from(&out_path).join(Pdfium::pdfium_platform_library_name());
    let bindings = Pdfium::bind_to_library(pdfium_libpath.display())
        .or_else(|_| Pdfium::bind_to_system_library())
        .unwrap();
    bindings
}
pub fn render_preview_page(
    data: &[u8],
    quailty: PDFQuailty,
) -> DynamicImage {

    let render_cfg = PdfBitmapConfig::new();
    let render_cfg = match quailty {
        PDFQuailty::High => render_cfg
            .set_target_width(2000)
            .set_maximum_height(2000),
        PDFQuailty::Medium => render_cfg,
        PDFQuailty::Low => render_cfg.thumbnail(50),
    }
    .rotate_if_landscape(PdfBitmapRotation::Degrees90, true);
    Pdfium::new(initialize_pdfium())
        .load_pdf_from_bytes(data, None)
        .unwrap()
        .pages()
        .get(0)
        .unwrap()
        .get_bitmap_with_config(&render_cfg)
        .unwrap()
        .as_image()
}


#[test]
fn test_multi_pdf_generate() {
    use tempdir::TempDir;
    let dir = TempDir::new("arklib_test").unwrap();
    let tmp_path = dir.path();
    println!("temp path: {}", tmp_path.display());
    for i in 0..2 {
        use std::{fs::File, io::Read};
        let mut pdf_reader = File::open("tests/test.pdf").unwrap();

        let mut bytes = Vec::new();
        pdf_reader.read_to_end(&mut bytes).unwrap();
        println!("Rendering {}", &i);
        let img = render_preview_page(bytes.as_slice(), PDFQuailty::Low);
        img.save(tmp_path.join(format!("test{}.png", &i)))
            .expect("cannot save image");
    }
}
