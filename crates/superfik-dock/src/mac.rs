//! AppKit: the image becomes `NSApplication.applicationIconImage`.

#![allow(unsafe_code)]

use objc2::AllocAnyThread;
use objc2_app_kit::{NSApplication, NSBitmapImageRep, NSDeviceRGBColorSpace, NSImage};
use objc2_foundation::{MainThreadMarker, NSSize};

use crate::{Image, Tile};

#[derive(Debug)]
pub struct MacDock {
    mtm: MainThreadMarker,
}

impl MacDock {
    pub fn new() -> Option<Self> {
        MainThreadMarker::new().map(|mtm| Self { mtm })
    }
}

impl Tile for MacDock {
    fn set(&mut self, image: &Image) {
        let (width, height) = (image.width as isize, image.height as isize);
        // SAFETY: a null `planes` asks AppKit to allocate the bitmap; the
        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                width,
                height,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                width * 4,
                32,
            )
        };
        let Some(rep) = rep else {
            tracing::warn!("dock: could not allocate the icon bitmap");
            return;
        };
        let premultiplied = image.premultiplied();
        let stride = rep.bytesPerRow() as usize;
        let row = image.width as usize * 4;
        // SAFETY: `bitmapData` points at `height × bytesPerRow` bytes owned
        unsafe {
            let data = rep.bitmapData();
            if data.is_null() {
                tracing::warn!("dock: the icon bitmap has no storage");
                return;
            }
            for y in 0..image.height as usize {
                std::ptr::copy_nonoverlapping(premultiplied.as_ptr().add(y * row), data.add(y * stride), row);
            }
        }
        let ns_image =
            NSImage::initWithSize(NSImage::alloc(), NSSize::new(f64::from(image.width), f64::from(image.height)));
        ns_image.addRepresentation(&rep);
        let app = NSApplication::sharedApplication(self.mtm);
        // SAFETY: called on the main thread with a live image.
        unsafe { app.setApplicationIconImage(Some(&ns_image)) };
    }
}
