pub(crate) mod frequency_v2;
pub(crate) mod jpeg_dct;

use crate::common::{RobustWatermarkOptions, TileAttempt};
use image::RgbImage;

pub(crate) trait WatermarkBackend {
    fn name(&self) -> &'static str;

    fn capacity_bits(&self, tile_size: u32, band_count: usize) -> usize;

    fn embed(
        &self,
        image: &RgbImage,
        origins: &[(u32, u32)],
        key: &str,
        bits: &[bool],
        options: &RobustWatermarkOptions,
    ) -> RgbImage;

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    fn decode_tile(
        &self,
        gray: &[f32],
        width: u32,
        height: u32,
        origin_x: u32,
        origin_y: u32,
        candidate_tile: u32,
        base_tile_size: u32,
        scale: f32,
        key: &str,
        bit_len: usize,
        band_count: usize,
    ) -> TileAttempt;
}
