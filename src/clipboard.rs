//! A picture to Windows' clipboard (right click on a slide or a mosaic tile), as a DIB that every program pastes.

use std::ffi::c_void;

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(owner: *mut c_void) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, data: *mut c_void) -> *mut c_void;
    fn CloseClipboard() -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalLock(memory: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(memory: *mut c_void) -> i32;
    fn GlobalFree(memory: *mut c_void) -> *mut c_void;
}

const CF_DIB: u32 = 8;
const GMEM_MOVEABLE: u32 = 2;

/// A 32-bit DIB of the picture: the header, then the rows from the bottom up in BGRA.
fn dib(image: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = image.dimensions();
    let mut out = Vec::with_capacity(40 + (w * h * 4) as usize);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]); // BI_RGB, and the sizes and colours left to the reader
    for row in image.rows().rev() {
        for p in row {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

/// Puts the picture in `bytes` (any format the image crate reads) on the clipboard.
pub fn copy_picture(bytes: &[u8]) -> Result<(), String> {
    let image = image::load_from_memory(bytes).map_err(|_| "not a picture".to_string())?.to_rgba8();
    let data = dib(&image);
    // SAFETY: the memory is ours until SetClipboardData takes it; it is freed only if that fails.
    unsafe {
        // Another program may hold the clipboard for a moment.
        if !(0..10).any(|_| OpenClipboard(std::ptr::null_mut()) != 0 || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        }) {
            return Err("the clipboard is busy".into());
        }
        EmptyClipboard();
        let memory = GlobalAlloc(GMEM_MOVEABLE, data.len());
        let ok = !memory.is_null() && {
            let at = GlobalLock(memory) as *mut u8;
            !at.is_null() && {
                std::ptr::copy_nonoverlapping(data.as_ptr(), at, data.len());
                GlobalUnlock(memory);
                !SetClipboardData(CF_DIB, memory).is_null()
            }
        };
        if !ok && !memory.is_null() {
            GlobalFree(memory);
        }
        CloseClipboard();
        if ok { Ok(()) } else { Err("couldn't use the clipboard".into()) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn dib_is_bottom_up_bgra() {
        let mut image = image::RgbaImage::new(1, 2);
        image.put_pixel(0, 0, image::Rgba([1, 2, 3, 4]));
        image.put_pixel(0, 1, image::Rgba([5, 6, 7, 8]));
        let dib = super::dib(&image);
        assert_eq!(dib.len(), 40 + 8);
        assert_eq!(&dib[40..], &[7, 6, 5, 8, 3, 2, 1, 4]);
    }
}
