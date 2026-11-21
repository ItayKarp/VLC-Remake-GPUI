fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon/icon.ico");

    // Embed the app icon into the Windows executable. GPUI loads icon resource
    // ID 1 for the window/taskbar icon, which is what `set_icon` uses.
    #[cfg(windows)]
    {
        let icon = std::path::Path::new("assets/app-icon/icon.ico");
        if !icon.exists() {
            println!("cargo:warning=assets/app-icon/icon.ico missing; run `cargo run --example gen_icons`");
        } else if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/app-icon/icon.ico")
                .set("ProductName", "VLC")
                .set("FileDescription", "VLC media player");
            res.compile().expect("failed to embed Windows resources");
        }
    }
}
