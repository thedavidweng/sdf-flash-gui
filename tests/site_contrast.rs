fn color(css: &str, rule: &str) -> [f64; 3] {
    let value = css.split_once(rule).unwrap().1.split('#').nth(1).unwrap();
    let hex = &value[..6];
    std::array::from_fn(|i| {
        let channel = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap() as f64 / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    })
}

fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let luminance = |c: [f64; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn secondary_text_remains_readable_on_dark_surfaces() {
    let css = include_str!("../site/style.css");
    let dim = color(css, "--dim:");
    let download = color(css, ".dl a span {");
    for background in [
        color(css, "--bg:"),
        color(css, "--bg-raise:"),
        color(css, ".titlebar {"),
    ] {
        assert!(contrast(dim, background) >= 4.5);
    }
    assert!(contrast(download, color(css, "--widget:")) >= 4.5);
}
