use std::{collections::HashSet, env, path::PathBuf, str::FromStr};

use flate2::read::GzDecoder;
use fs_extra::file::CopyOptions;

use tar::Archive;
use target_lexicon::{Architecture, Environment, OperatingSystem, Triple};
const PDFIUM_VERSION: &str = "5104";

fn main() {
    let t = env::var("TARGET").unwrap();
    let target = Triple::from_str(t.as_str()).unwrap();
    let out_dir = env::var_os("OUT_DIR").unwrap();

    // Avoid duplicate download
    if !fs_extra::dir::ls(&out_dir, &HashSet::new())
        .unwrap()
        .items
        .is_empty()
    {
        return;
    }

    let mut name = vec!["pdfium"];
    match target.environment {
        Environment::Android | Environment::Androideabi => {
            // The prebuilt pdfium library is included in the arklib-android, no
            // more fetching work needed.
            name.push("android");
            return;
        }
        _ => {}
    }

    match target.operating_system {
        OperatingSystem::Windows => name.push("win"),
        OperatingSystem::Linux => {
            if target.environment != Environment::Android {
                name.push("linux")
            }
        }
        OperatingSystem::Ios => name.push("ios"),
        OperatingSystem::MacOSX {
            major: 11,
            minor: 0,
            patch: 0,
        } => name.push("mac"),
        _ => {}
    }

    match target.architecture {
        Architecture::Arm(_) => name.push("arm"),
        Architecture::Aarch64(_) => name.push("arm64"),
        Architecture::X86_32(_) => name.push("x86"),
        Architecture::X86_64 => name.push("x64"),
        _ => {}
    }
    dbg!(&name);

    let filename = name.join("-").to_string();
    let url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/{}/{}.tgz",
        PDFIUM_VERSION, filename
    );

    let request = ureq::get(url.as_str())
        .call()
        .unwrap()
        .into_reader();
    let ar = GzDecoder::new(request);
    let mut ar = Archive::new(ar);
    ar.unpack(&out_dir).unwrap();
    match target.operating_system {
        OperatingSystem::Windows => fs_extra::file::move_file(
            PathBuf::from(&out_dir)
                .join("bin")
                .join("pdfium.dll"),
            PathBuf::from(&out_dir).join("pdfium.dll"),
            &CopyOptions::new(),
        )
        .unwrap(),
        OperatingSystem::Ios
        | OperatingSystem::MacOSX {
            major: 11,
            minor: 0,
            patch: 0,
        } => fs_extra::file::move_file(
            PathBuf::from(&out_dir)
                .join("bin")
                .join("libpdfium.dylib"),
            PathBuf::from(&out_dir).join("libpdfium.dylib"),
            &CopyOptions::new(),
        )
        .unwrap(),
        _ => fs_extra::file::move_file(
            PathBuf::from(&out_dir)
                .join("lib")
                .join("libpdfium.so"),
            PathBuf::from(&out_dir).join("libpdfium.so"),
            &CopyOptions::new(),
        )
        .unwrap(),
    };
    println!("cargo:rerun-if-changed=build.rs");
}
