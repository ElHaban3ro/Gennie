use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/RemindersBridge.swift");
    println!("cargo:rerun-if-changed=native/Info.plist");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("gennie-reminders-bridge");
    let status = Command::new("swiftc")
        .args([
            "-O",
            "-framework",
            "EventKit",
            "native/RemindersBridge.swift",
            "-o",
        ])
        .arg(&output)
        .args([
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__info_plist",
            "-Xlinker",
            "native/Info.plist",
        ])
        .status()
        .expect("No se pudo ejecutar swiftc para Recordatorios");
    assert!(status.success(), "No se pudo compilar el puente EventKit");
    println!(
        "cargo:rustc-env=GENNIE_REMINDERS_BRIDGE={}",
        output.display()
    );
}
