//! Lettura per le anteprime e i visori: le immagini che Qt non sa leggere
//! (WebP, TIFF, BMP: nel runtime ci sono solo jpeg, png, gif, svg, ico) e il
//! testo dei file per il visore.

use std::fs::File;
use std::io::Read;

use crate::ffi::Picture;

pub fn decode_picture(path: &str, max: u32) -> Result<Picture, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| e.to_string())?;
    let img = if img.width() > max || img.height() > max { img.thumbnail(max, max) } else { img };
    let rgba = img.into_rgba8();
    Ok(Picture { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() })
}

/// Al più `max_bytes` del file come testo. Un file con byte nulli nel primo
/// blocco si considera binario (come fa `grep`); i byte non UTF-8 diventano «�».
pub fn read_text(path: &str, max_bytes: u64) -> Result<String, String> {
    let mut buf = Vec::new();
    File::open(path)
        .and_then(|f| f.take(max_bytes).read_to_end(&mut buf))
        .map_err(|e| e.to_string())?;
    if buf.iter().take(8192).any(|&b| b == 0) {
        return Err("binary".into());
    }
    // l'ultimo carattere può essere tagliato a metà dal limite: si toglie
    let mut end = buf.len();
    while end > 0 && end > buf.len().saturating_sub(4) && std::str::from_utf8(&buf[..end]).is_err() {
        end -= 1;
    }
    let text = if std::str::from_utf8(&buf[..end]).is_ok() { &buf[..end] } else { &buf[..] };
    Ok(String::from_utf8_lossy(text).into_owned())
}
