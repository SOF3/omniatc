use std::path::Path;
use std::{array, iter, ops};

use anyhow::{Context, Result};
use bevy_math::UVec3;
use image::RgbImage;

const SMOOTHING_HALF_WIDTH_PLUS_ONE: usize = 3;
const PER_CHANNEL_THRESHOLD: u8 = 2;
const BYTES_THRESHOLD: usize = 10;

/// Compare two images and save the diff image to `diff_path` if they differ.
pub fn compare_images(
    mut baseline: RgbImage,
    mut actual: RgbImage,
    diff_path: &Path,
) -> Result<()> {
    if baseline.dimensions() != actual.dimensions() {
        anyhow::bail!(
            "Screenshot dimensions mismatch: expected {}x{}, got {}x{}",
            baseline.width(),
            baseline.height(),
            actual.width(),
            actual.height()
        );
    }

    smoothen_image::<SMOOTHING_HALF_WIDTH_PLUS_ONE>(&mut baseline);
    smoothen_image::<SMOOTHING_HALF_WIDTH_PLUS_ONE>(&mut actual);

    let mut diff_image = baseline;
    let mut diff_bytes = 0;
    for (byte1, byte2) in iter::zip(&mut *diff_image, actual.as_raw()) {
        let delta = byte1.abs_diff(*byte2);
        if delta > PER_CHANNEL_THRESHOLD {
            diff_bytes += 1;
        }

        *byte1 ^= *byte2;
    }

    if diff_bytes < BYTES_THRESHOLD {
        return Ok(());
    }

    diff_image
        .save_with_format(diff_path, image::ImageFormat::Png)
        .context("Failed to save screenshot diff image")?;
    anyhow::bail!(
        "Screenshot mismatch, {diff_bytes} / {} bytes differ",
        actual.width() * actual.height() * 3
    );
}

/// Take the averaging window of each pixel in the image.
/// The window size is `HALF_WIDTH_PLUS_ONE * 2 - 1`,
/// e.g. `3` would take the 5x5 square around a pixel (with 2 extra pixels on each side).
pub fn smoothen_image<const HALF_WIDTH_PLUS_ONE: usize>(image: &mut RgbImage) {
    let mut sums =
        image.pixels().map(|pixel| UVec3::from(pixel.0.map(u32::from))).collect::<Vec<_>>();

    for x in 0..image.width() {
        sliding_window_sum::<_, HALF_WIDTH_PLUS_ONE>(
            image.height() as usize,
            DpStateBySliceIndexFn {
                slice:    &mut sums,
                index_fn: |y| x as usize + y * image.width() as usize,
            },
        );
    }
    for y in 0..image.height() {
        let row = &mut sums
            [y as usize * image.width() as usize..(y as usize + 1) * image.width() as usize];

        sliding_window_sum::<_, HALF_WIDTH_PLUS_ONE>(
            image.width() as usize,
            DpStateBySliceIndexFn { slice: row, index_fn: |x| x },
        );

        #[expect(clippy::cast_possible_truncation, reason = "small constant")]
        let half_width = HALF_WIDTH_PLUS_ONE as u32 - 1;
        let window_height_above = y.min(half_width);
        let window_height_below = (image.height() - 1 - y).min(half_width);
        let window_height = window_height_above + 1 + window_height_below;

        for x in 0..half_width {
            row[x as usize] /= window_height * (half_width + 1 + x);
        }
        for item in &mut row[half_width as usize..(image.width() - half_width) as usize] {
            *item /= window_height * (half_width * 2 + 1);
        }
        for x in image.width() - half_width..image.width() {
            row[x as usize] /= window_height * (half_width + 1 + (image.width() - x));
        }
    }

    for (pixel, uv) in iter::zip(image.pixels_mut(), sums) {
        pixel.0 = uv.to_array().map(|b| u8::try_from(b).unwrap_or(u8::MAX));
    }
}

trait DpState {
    type Item;

    fn get(&mut self, i: usize) -> &mut Self::Item;
}

struct DpStateBySliceIndexFn<'a, T, F> {
    slice:    &'a mut [T],
    index_fn: F,
}

impl<T, F> DpState for DpStateBySliceIndexFn<'_, T, F>
where
    F: Fn(usize) -> usize,
{
    type Item = T;

    fn get(&mut self, i: usize) -> &mut T { &mut self.slice[(self.index_fn)(i)] }
}

fn sliding_window_sum<
    T: Default + ops::AddAssign + ops::SubAssign + Copy,
    const HALF_WIDTH_PLUS_ONE: usize,
>(
    total: usize,
    mut dp: impl DpState<Item = T>,
) {
    let half_width = HALF_WIDTH_PLUS_ONE - 1;
    let mut buffer: [T; HALF_WIDTH_PLUS_ONE] = array::from_fn(|_| T::default());
    let mut buffer_read_next = 0;
    let mut buffer_write_next = 0;

    let mut state = T::default();
    for i in 0..half_width {
        state += *dp.get(i);
    }

    for cursor in 0..total {
        if cursor + half_width < total {
            state += *dp.get(cursor + half_width);
        }

        if cursor > half_width {
            state -= buffer[buffer_read_next];
            buffer_read_next = (buffer_read_next + 1) % buffer.len();
        }

        let ptr = dp.get(cursor);
        buffer[buffer_write_next] = *ptr;
        buffer_write_next = (buffer_write_next + 1) % buffer.len();
        *ptr = state;
    }
}

#[cfg(test)]
mod tests {
    use crate::DpStateBySliceIndexFn;

    #[test]
    fn test_sliding_window_sum() {
        let mut dp: [u64; 10] = [0, 10, 2, 30, 4, 50, 6, 70, 8, 90];
        super::sliding_window_sum::<u64, 3>(
            dp.len(),
            DpStateBySliceIndexFn { slice: &mut dp, index_fn: |i| i },
        );
        assert_eq!(dp, [12, 42, 46, 96, 92, 160, 138, 224, 174, 168]);
    }
}
