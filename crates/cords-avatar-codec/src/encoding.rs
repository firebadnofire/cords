//! Incremental lossless encoding using the same pinned, memory-limited sandbox.
use super::{Host, ImageInfo, instantiate};
use anyhow::{Result, ensure};
use std::time::Instant;
use wasmtime::{Memory, Store, TypedFunc};

pub struct Encoder {
    store: Store<Host>,
    memory: Memory,
    frame: TypedFunc<(u32, u32, u32, u32), i32>,
    read: TypedFunc<(u32, u32), i32>,
    status: TypedFunc<(), i32>,
    pixels: u32,
    drain: u32,
    size: u32,
    output: Vec<u8>,
    finished: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lossless_encoder_round_trips_pixels_and_exact_animation_timing() -> Result<()> {
        let info = ImageInfo { width: 2, height: 2, bits: 8, frames: 2, loops: 3, ticks_per_second_numerator: 300, ticks_per_second_denominator: 1, durations: vec![0, 7] };
        let mut encoder = Encoder::new(&info, true, Instant::now() + std::time::Duration::from_secs(120))?;
        let pixels = [31, 81, 191, 255].repeat(4);
        encoder.push(&pixels, 0, false)?;
        encoder.push(&pixels, 7, true)?;
        let bytes = encoder.finish()?;
        assert_eq!(super::super::validate(&bytes)?, info);
        let mut decoder = super::super::playback::Decoder::new(&bytes)?;
        assert_eq!(decoder.next_frame()?.unwrap().pixels, pixels);
        assert!(decoder.next_frame()?.unwrap().last);
        Ok(())
    }
}
impl std::fmt::Debug for Encoder {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Encoder").field("finished", &self.finished).finish_non_exhaustive()
    }
}
impl Encoder {
    /// # Errors
    /// Rejects invalid geometry, allocations, and exhausted processing budgets.
    pub fn new(info: &ImageInfo, animated: bool, deadline: Instant) -> Result<Self> {
        ensure!(info.width > 0 && info.width <= 1024 && info.height > 0 && info.height <= 1024 && matches!(info.bits, 8 | 16), "Invalid avatar output geometry");
        ensure!(Instant::now() < deadline, "Avatar processing exceeded 120 seconds");
        let (mut store, instance, memory) = instantiate(20_000_000_000)?;
        store.data_mut().deadline = deadline;
        let alloc = instance.get_typed_func::<u32, u32>(&mut store, "h")?;
        let begin = instance.get_typed_func::<(u32, u32, u32, u32, u32, u32, u32), i32>(&mut store, "o")?;
        let size = info.width * info.height * 4 * (info.bits / 8);
        let pixels = alloc.call(&mut store, size)?;
        let drain = alloc.call(&mut store, 65536)?;
        ensure!(pixels != 0 && drain != 0, "Avatar encoder allocation failed");
        ensure!(begin.call(&mut store, (info.width, info.height, info.bits, u32::from(animated), info.ticks_per_second_numerator, info.ticks_per_second_denominator, info.loops))? >= 0, "Avatar encoder rejected metadata");
        Ok(Self {
            frame: instance.get_typed_func(&mut store, "p")?,
            read: instance.get_typed_func(&mut store, "q")?,
            status: instance.get_typed_func(&mut store, "r")?,
            store, memory, pixels, drain, size, output: Vec::new(), finished: false,
        })
    }
    /// # Errors
    /// Rejects invalid frames, output over 5 MB, and sandbox/deadline failures.
    pub fn push(&mut self, pixels: &[u8], duration: u32, last: bool) -> Result<()> {
        ensure!(!self.finished && pixels.len() == self.size as usize, "Invalid encoder frame");
        self.memory.write(&mut self.store, self.pixels as usize, pixels)?;
        ensure!(self.frame.call(&mut self.store, (self.pixels, self.size, duration, u32::from(last)))? >= 0, "Avatar frame encoding failed");
        loop {
            let count = self.read.call(&mut self.store, (self.drain, 65536))?;
            ensure!((0..=65536).contains(&count), "Avatar encoder output failed");
            let length = self.output.len();
            ensure!(length + count as usize <= 5_000_000, "Cropped image exceeds 5 MB; zoom in or select a smaller image");
            self.output.resize(length + count as usize, 0);
            self.memory.read(&self.store, self.drain as usize, &mut self.output[length..])?;
            let status = self.status.call(&mut self.store, ())?;
            ensure!(status == 0 || (status == 2 && count > 0), "Avatar encoder made no progress");
            if status == 0 { break; }
        }
        self.finished = last;
        Ok(())
    }
    /// # Errors
    /// Rejects unfinished or empty output.
    pub fn finish(self) -> Result<Vec<u8>> {
        ensure!(self.finished && !self.output.is_empty(), "Avatar encoding is incomplete");
        Ok(self.output)
    }
}
