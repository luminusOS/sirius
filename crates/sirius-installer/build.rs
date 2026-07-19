//! Build-time i18n: compile the gettext catalogs declared in `po/LINGUAS`
//! into `$OUT_DIR/locale` (embedded path used by dev runs) and copy them into
//! `data/locale` at the workspace root so `generate-rpm` can package them.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let po_dir = manifest_dir.join("../../po");
    let out_locale = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("locale");
    let data_locale = manifest_dir.join("../../data/locale");

    let linguas_path = po_dir.join("LINGUAS");
    println!("cargo:rerun-if-changed={}", linguas_path.display());
    let linguas = std::fs::read_to_string(&linguas_path).expect("po/LINGUAS must be readable");

    for lang in linguas
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let po = po_dir.join(format!("{lang}.po"));
        println!("cargo:rerun-if-changed={}", po.display());
        for base in [&out_locale, &data_locale] {
            let dest_dir = base.join(lang).join("LC_MESSAGES");
            std::fs::create_dir_all(&dest_dir).expect("locale dir must be creatable");
            let status = Command::new("msgfmt")
                .arg("--check")
                .arg("-o")
                .arg(dest_dir.join("sirius.mo"))
                .arg(&po)
                .status()
                .expect("msgfmt must be available to compile gettext catalogs");
            assert!(status.success(), "msgfmt failed for {po:?}");
        }
    }

    println!(
        "cargo:rustc-env=SIRIUS_DEV_LOCALEDIR={}",
        out_locale.display()
    );
}
