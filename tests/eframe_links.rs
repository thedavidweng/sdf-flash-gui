#[test]
fn eframe_links_feature_pulls_webbrowser() {
    let manifest = include_str!("../Cargo.toml");
    let features = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("eframe ="))
        .expect("eframe dependency");
    assert!(
        features.contains("\"links\""),
        "native hyperlinks need the eframe links feature when default features are off"
    );

    let lock = include_str!("../Cargo.lock");
    assert!(
        lock.contains("name = \"webbrowser\""),
        "egui-winit opens hyperlinks only when webbrowser is in the lockfile"
    );
}
