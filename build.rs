fn main() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml is readable");
    let product_name = manifest
        .lines()
        .find_map(|line| line.strip_prefix("product-name = \"")?.strip_suffix('"'))
        .expect("product-name under [package.metadata.packager]");
    println!("cargo:rustc-env=PRODUCT_NAME={product_name}");
    println!("cargo:rerun-if-changed=Cargo.toml");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set("FileDescription", product_name);
        res.set("ProductName", product_name);
        res.set("OriginalFilename", "sdf-flash-gui.exe");
        if let Err(err) = res.compile() {
            eprintln!("Windows resource compilation failed: {err}");
            std::process::exit(1);
        }
    }
}
