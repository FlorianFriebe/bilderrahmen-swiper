//! Windows Imaging Component: Fallback-Decoder (u. a. für HEIC, wenn die
//! HEIF-Bilderweiterungen von Microsoft installiert sind).

use std::path::Path;

use image::{DynamicImage, RgbImage};
use windows::core::HSTRING;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

use crate::imaging::Decoded;

pub fn decode(path: &Path) -> Result<Decoded, String> {
    unsafe { decode_inner(path).map_err(|e| e.message().to_string()) }
}

unsafe fn decode_inner(path: &Path) -> windows::core::Result<Decoded> {
    // Mehrfaches Initialisieren pro Thread ist erlaubt.
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    let factory: IWICImagingFactory =
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
    let decoder = factory.CreateDecoderFromFilename(
        &HSTRING::from(path.as_os_str()),
        None,
        GENERIC_READ,
        WICDecodeMetadataCacheOnDemand,
    )?;
    let frame = decoder.GetFrame(0)?;
    let conv = factory.CreateFormatConverter()?;
    conv.Initialize(
        &frame,
        &GUID_WICPixelFormat24bppRGB,
        WICBitmapDitherTypeNone,
        None,
        0.0,
        WICBitmapPaletteTypeCustom,
    )?;
    let (mut w, mut h) = (0u32, 0u32);
    conv.GetSize(&mut w, &mut h)?;
    let stride = w * 3;
    let mut buf = vec![0u8; (stride as usize) * (h as usize)];
    conv.CopyPixels(std::ptr::null(), stride, &mut buf)?;
    let img = RgbImage::from_raw(w, h, buf).ok_or_else(|| windows::core::Error::empty())?;
    Ok(Decoded {
        img: DynamicImage::ImageRgb8(img),
        icc: None,
    })
}
