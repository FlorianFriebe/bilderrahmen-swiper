fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.set("ProductName", "Bilderrahmen-Swiper");
            res.set("FileDescription", "Bilderrahmen-Swiper");
            if let Err(e) = res.compile() {
                println!("cargo:warning=Icon konnte nicht eingebettet werden: {e}");
            }
        }
    }
}
