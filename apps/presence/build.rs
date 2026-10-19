fn main() {
    slint_build::compile("ui/presence.slint").expect("UI Slint inválida");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR ausente"));
        let source = "native/face_wallpaper.m";
        let object = out.join("face_wallpaper.o");
        let archive = out.join("libsoc_face_wallpaper.a");
        let compile = std::process::Command::new("xcrun")
            .args([
                "--sdk",
                "macosx",
                "clang",
                "-fobjc-arc",
                "-O2",
                "-c",
                source,
                "-o",
            ])
            .arg(&object)
            .status()
            .expect("No se pudo iniciar clang");
        assert!(compile.success(), "No se pudo compilar face_wallpaper.m");
        let link = std::process::Command::new("xcrun")
            .args(["libtool", "-static", "-o"])
            .arg(&archive)
            .arg(&object)
            .status()
            .expect("No se pudo iniciar libtool");
        assert!(link.success(), "No se pudo enlazar face_wallpaper.m");
        println!("cargo:rerun-if-changed={source}");
        println!("cargo:rustc-link-search=native={}", out.display());
        println!("cargo:rustc-link-lib=static=soc_face_wallpaper");
        for framework in [
            "AppKit",
            "QuartzCore",
            "CoreGraphics",
            "CoreImage",
            "ImageIO",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
}
