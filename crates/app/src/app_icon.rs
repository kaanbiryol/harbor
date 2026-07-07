#[cfg(target_os = "macos")]
pub(crate) fn install() {
    use cocoa::{
        appkit::{NSApplication, NSImage},
        base::{id, nil},
        foundation::{NSAutoreleasePool, NSData, NSUInteger},
    };
    use std::os::raw::c_void;

    const HARBOR_ICON: &[u8] = include_bytes!("../assets/harbor.icns");

    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let data = NSData::dataWithBytes_length_(
            nil,
            HARBOR_ICON.as_ptr().cast::<c_void>(),
            HARBOR_ICON.len() as NSUInteger,
        );
        let image: id = NSImage::initWithData_(NSImage::alloc(nil), data);

        if image != nil {
            let app = NSApplication::sharedApplication(nil);
            app.setApplicationIconImage_(image);
            image.autorelease();
        }

        pool.drain();
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn install() {}
