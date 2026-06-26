fn main() {
    #[cfg(windows)]
    {
        // Keep in sync with src/branding.rs and Cargo.toml `product-name`.
        const DISPLAY_NAME: &str = "SDF Flash GUI";
        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", DISPLAY_NAME);
        res.set("ProductName", DISPLAY_NAME);
        res.set("OriginalFilename", "sdf-flash-gui.exe");
        if let Err(err) = res.compile() {
            eprintln!("Windows resource compilation failed: {err}");
            std::process::exit(1);
        }
    }
}
