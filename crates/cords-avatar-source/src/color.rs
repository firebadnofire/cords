//! Convert supported RGB profiles to sRGB before geometric processing. Integer
//! precision and alpha are retained. Unsupported profiles fail explicitly.
use moxcms::{ColorProfile, DataColorSpace, InPlaceTransformExecutor, Layout, ParsingOptions};
use std::sync::Arc;

pub(super) enum ColorTransform {
    None,
    Eight(Arc<dyn InPlaceTransformExecutor<u8> + Send + Sync>),
    Sixteen(Arc<dyn InPlaceTransformExecutor<u16> + Send + Sync>),
}

impl ColorTransform {
    pub(super) fn new(icc: &[u8], bits: u8) -> Result<Self, String> {
        if icc.is_empty() {
            return Ok(Self::None);
        }
        let profile = ColorProfile::new_from_slice_with_options(
            icc,
            ParsingOptions {
                max_profile_size: super::MAX_METADATA + 1,
                max_allowed_clut_size: super::MAX_METADATA,
                max_allowed_trc_size: 65536,
            },
        )
        .map_err(|e| format!("Unsupported or invalid image color profile: {e}"))?;
        Self::from_profile(&profile, bits)
    }

    pub(super) fn from_profile(profile: &ColorProfile, bits: u8) -> Result<Self, String> {
        if profile.color_space != DataColorSpace::Rgb {
            return Err(
                "Unsupported image color profile: only RGB ICC profiles are supported".into(),
            );
        }
        let destination = ColorProfile::new_srgb();
        let options = moxcms::TransformOptions {
            prefer_fixed_point: false,
            ..Default::default()
        };
        match bits {
            8 => profile
                .create_in_place_transform_8bit(Layout::Rgba, &destination, options)
                .map(Self::Eight),
            16 => profile
                .create_in_place_transform_16bit(Layout::Rgba, &destination, options)
                .map(Self::Sixteen),
            _ => return Err("Unsupported color sample precision".into()),
        }
        .map_err(|e| format!("Cannot process image color profile: {e}"))
    }

    pub(super) fn apply(&self, pixels: &mut [u8]) -> Result<(), String> {
        match self {
            Self::None => Ok(()),
            Self::Eight(transform) => transform
                .transform(pixels)
                .map_err(|e| format!("Image color conversion failed: {e}")),
            Self::Sixteen(transform) => {
                let mut samples: Vec<u16> = pixels
                    .chunks_exact(2)
                    .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                    .collect();
                transform
                    .transform(&mut samples)
                    .map_err(|e| format!("Image color conversion failed: {e}"))?;
                for (bytes, sample) in pixels.chunks_exact_mut(2).zip(samples) {
                    bytes.copy_from_slice(&sample.to_le_bytes());
                }
                Ok(())
            }
        }
    }
}

pub(super) fn png(info: &png::Info<'_>, bits: u8) -> Result<ColorTransform, String> {
    if let Some(profile) = info.icc_profile.as_deref() {
        return ColorTransform::new(profile, bits);
    }
    if info.coding_independent_code_points.is_some() {
        return Err("Unsupported PNG CICP color representation; use an RGB ICC profile".into());
    }
    if info.srgb.is_some() {
        return Ok(ColorTransform::None);
    }
    let srgb =
        png::SourceChromaticities::new((0.3127, 0.329), (0.64, 0.33), (0.3, 0.6), (0.15, 0.06));
    if info.chrm_chunk.is_some_and(|value| value != srgb) {
        return Err("Unsupported PNG chromaticities without an RGB ICC profile".into());
    }
    let Some(gamma) = info.gama_chunk else {
        return Ok(ColorTransform::None);
    };
    let gamma = gamma.into_value();
    if !gamma.is_finite() || gamma <= 0.0 {
        return Err("Invalid PNG source gamma".into());
    }
    let mut profile = ColorProfile::new_srgb();
    profile.cicp = None;
    let curve = moxcms::ToneReprCurve::Parametric(vec![gamma.recip()]);
    profile.red_trc = Some(curve.clone());
    profile.green_trc = Some(curve.clone());
    profile.blue_trc = Some(curve);
    ColorTransform::from_profile(&profile, bits)
}
