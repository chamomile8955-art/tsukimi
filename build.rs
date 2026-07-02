fn main() {
    const WINDOWS_ICON: &str = "resources/icons/tsukimi.ico";

    println!("cargo:rerun-if-changed={WINDOWS_ICON}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(WINDOWS_ICON);
        resource
            .compile()
            .expect("failed to embed the Tsukimi application icon");
    }
}
