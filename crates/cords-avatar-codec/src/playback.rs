//! Incremental pixels for native image consumers of the pinned sandbox decoder.
//! A decoder retains one canvas, never a decoded animation or frame cache.
use super::{Host, MAX_INPUT, instantiate};
use anyhow::{Result, bail, ensure};
use std::time::{Duration, Instant};
use wasmtime::{Memory, Store, TypedFunc};

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub duration_ticks: u32,
    pub ticks_numerator: u32,
    pub ticks_denominator: u32,
    pub loops: u32,
    pub last: bool,
    pub pixels: Vec<u8>,
}

pub struct Decoder {
    store: Store<Host>,
    memory: Memory,
    next: TypedFunc<(), i32>,
    info: TypedFunc<i32, u32>,
    pixels: TypedFunc<(u32, u32), i32>,
    pointer: u32,
    width: u32,
    height: u32,
    bits: u32,
    numerator: u32,
    denominator: u32,
    loops: u32,
    animated: bool,
    frames: u32,
    ticks: u64,
    done: bool,
    remaining: Duration,
}
impl std::fmt::Debug for Decoder {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Decoder")
            .field("frames", &self.frames)
            .finish_non_exhaustive()
    }
}
impl Decoder {
    /// Original animation flag and loop count, including single-frame animations.
    #[must_use]
    pub fn animation(&self) -> (bool, u32) { (self.animated, self.loops) }
    /// # Errors
    /// Rejects excessive input, dimensions, precision, invalid headers or limits.
    pub fn new(input: &[u8]) -> Result<Self> {
        ensure!(
            !input.is_empty() && input.len() <= MAX_INPUT,
            "Avatar exceeds 5,000,000 bytes"
        );
        let (mut store, instance, memory) = instantiate(2_000_000_000)?;
        let started = Instant::now();
        let alloc = instance.get_typed_func::<u32, u32>(&mut store, "h")?;
        let begin = instance.get_typed_func::<(u32, u32, i32), i32>(&mut store, "s")?;
        let next = instance.get_typed_func::<(), i32>(&mut store, "t")?;
        let info = instance.get_typed_func::<i32, u32>(&mut store, "u")?;
        let pixels = instance.get_typed_func::<(u32, u32), i32>(&mut store, "v")?;
        let input_pointer = alloc.call(&mut store, u32::try_from(input.len())?)?;
        ensure!(input_pointer != 0, "Avatar input allocation failed");
        memory.write(&mut store, input_pointer as usize, input)?;
        ensure!(
            begin.call(&mut store, (input_pointer, u32::try_from(input.len())?, 1))? >= 0,
            "Invalid JPEG XL header"
        );
        ensure!(next.call(&mut store, ())? == 1, "Missing JPEG XL metadata");
        let width = info.call(&mut store, 0)?;
        let height = info.call(&mut store, 1)?;
        let bits = info.call(&mut store, 2)?;
        ensure!(
            width > 0 && width <= 1024 && height > 0 && height <= 1024 && matches!(bits, 8 | 16),
            "Invalid avatar dimensions or precision"
        );
        let animated = info.call(&mut store, 3)? != 0;
        let numerator = if animated {
            info.call(&mut store, 4)?
        } else {
            1
        };
        let denominator = if animated {
            info.call(&mut store, 5)?
        } else {
            1
        };
        ensure!(numerator > 0 && denominator > 0, "Invalid avatar timebase");
        let loops = if animated {
            info.call(&mut store, 6)?
        } else {
            1
        };
        let pointer = alloc.call(&mut store, width * height * 4 * (bits / 8))?;
        ensure!(pointer != 0, "Avatar canvas allocation failed");
        Ok(Self {
            store,
            memory,
            next,
            info,
            pixels,
            pointer,
            width,
            height,
            bits,
            numerator,
            denominator,
            loops,
            animated,
            frames: 0,
            ticks: 0,
            done: false,
            remaining: Duration::from_secs(15).saturating_sub(started.elapsed()),
        })
    }
    /// Decode one composited frame, preserving zero-duration layers and precision.
    /// Waits between presentation frames do not consume the decoder's CPU deadline.
    /// # Errors
    /// Rejects malformed order, excessive frames/pixels/cycle time or work limits.
    pub fn next_frame(&mut self) -> Result<Option<Frame>> {
        if self.done {
            return Ok(None);
        }
        let started = Instant::now();
        self.store.data_mut().deadline = started + self.remaining;
        let result = self.decode_frame();
        self.remaining = self.remaining.saturating_sub(started.elapsed());
        if result.is_err() {
            self.done = true;
        }
        result
    }
    fn decode_frame(&mut self) -> Result<Option<Frame>> {
        let mut duration = None;
        let mut last = false;
        let size = self.width * self.height * 4 * (self.bits / 8);
        loop {
            ensure!(
                Instant::now() < self.store.data().deadline,
                "Avatar decode deadline exceeded"
            );
            match self.next.call(&mut self.store, ())? {
                2 => {
                    ensure!(
                        duration.is_none() && self.frames < 1000,
                        "Invalid avatar frame order/count"
                    );
                    self.frames += 1;
                    ensure!(
                        u64::from(self.width) * u64::from(self.height) * u64::from(self.frames)
                            <= 100_000_000,
                        "Avatar cumulative pixel limit"
                    );
                    let ticks = self.info.call(&mut self.store, 7)?;
                    self.ticks += u64::from(ticks);
                    ensure!(
                        u128::from(self.ticks) * u128::from(self.denominator)
                            <= 120 * u128::from(self.numerator),
                        "Avatar cycle exceeds 120 seconds"
                    );
                    duration = Some(ticks);
                    last = self.info.call(&mut self.store, 9)? == 1;
                }
                3 => ensure!(
                    duration.is_some()
                        && self.pixels.call(&mut self.store, (self.pointer, size))? >= 0,
                    "Invalid avatar canvas"
                ),
                4 => {
                    let duration_ticks =
                        duration.ok_or_else(|| anyhow::anyhow!("Avatar frame without header"))?;
                    let mut pixels = vec![0; size as usize];
                    self.memory
                        .read(&self.store, self.pointer as usize, &mut pixels)?;
                    if last {
                        ensure!(
                            self.next.call(&mut self.store, ())? == 0,
                            "Trailing JPEG XL data"
                        );
                        self.done = true;
                    }
                    return Ok(Some(Frame {
                        width: self.width,
                        height: self.height,
                        bits: self.bits,
                        duration_ticks,
                        ticks_numerator: self.numerator,
                        ticks_denominator: self.denominator,
                        loops: self.loops,
                        last,
                        pixels,
                    }));
                }
                _ => bail!("JPEG XL decode rejected image"),
            }
        }
    }
}
