#[cfg(target_os = "macos")]
pub(crate) fn install() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    const HARBOR_ICON: &[u8] = include_bytes!("../assets/harbor.icns");

    let Some(main_thread) = MainThreadMarker::new() else {
        tracing::warn!("skipping app icon install off the main thread");
        return;
    };
    let data = NSData::with_bytes(HARBOR_ICON);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        let app = NSApplication::sharedApplication(main_thread);
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn install() {}
