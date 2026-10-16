//! Restricted validation of the exact bundled libjxl WASM. No WASI, filesystem,
//! network, clock or randomness imports. Only bounded memory growth is permitted.
use anyhow::{Result, bail, ensure};
pub mod playback;
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};
use wasmtime::{
    Caller, Config, Engine, Instance, Linker, Memory, MemoryType, Module, Store, StoreLimits,
    StoreLimitsBuilder, UpdateDeadline,
};

const WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/cords-jxl.wasm"));
const HEAP: usize = 128 * 1024 * 1024;
const MAX_INPUT: usize = 5_000_000;
// Wasmtime's limiter does not cover host allocations. The validator retains no
// host canvas/output copies: at most the caller's 5 MB input and 4 KB error text.
// Reserve 64 MiB separately for runtime/compiled bookkeeping; no unbounded imports
// or logs. API callers separately bound body staging and validation concurrency.
const HOST_RESERVATION: usize = 64 * 1024 * 1024 + MAX_INPUT + 4096;
static COMPILED: OnceLock<std::result::Result<(Engine, Module), String>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub bits: u32,
    pub frames: u32,
    pub loops: u32,
    pub ticks_per_second_numerator: u32,
    pub ticks_per_second_denominator: u32,
    pub durations: Vec<u32>,
}

struct Host {
    limits: StoreLimits,
    memory: Option<Memory>,
    deadline: Instant,
}

fn compiled() -> Result<&'static (Engine, Module)> {
    COMPILED
        .get_or_init(|| {
            (|| -> Result<_> {
                let mut config = Config::new();
                config
                    .consume_fuel(true)
                    .epoch_interruption(true)
                    .max_wasm_stack(512 * 1024);
                let engine = Engine::new(&config)?;
                let module = Module::new(&engine, WASM)?;
                let timer_engine = engine.clone();
                std::thread::Builder::new()
                    .name("avatar-codec-clock".into())
                    .spawn(move || {
                        loop {
                            std::thread::sleep(Duration::from_millis(50));
                            timer_engine.increment_epoch();
                        }
                    })?;
                Ok((engine, module))
            })()
            .map_err(|error| format!("Avatar codec initialization failed: {error:#}"))
        })
        .as_ref()
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

/// Fully decode one bounded image, counting all coalesced zero-duration steps.
/// # Errors
/// Rejects malformed images, unsupported precision, excessive work, memory or time.
pub fn validate(input: &[u8]) -> Result<ImageInfo> {
    validate_bounded(input, 2_000_000_000)
}

fn validate_bounded(input: &[u8], fuel: u64) -> Result<ImageInfo> {
    ensure!(
        !input.is_empty() && input.len() <= MAX_INPUT,
        "Avatar exceeds 5,000,000 bytes"
    );
    ensure!(
        HEAP + HOST_RESERVATION < 512 * 1024 * 1024,
        "Invalid codec reservations"
    );
    decode_image(input, instantiate(fuel)?)
}

fn instantiate(fuel: u64) -> Result<(Store<Host>, Instance, Memory)> {
    let (engine, module) = compiled()?;
    let mut store = Store::new(
        engine,
        Host {
            limits: StoreLimitsBuilder::new()
                .memory_size(HEAP)
                .table_elements(16384)
                .instances(1)
                .memories(1)
                .tables(1)
                .build(),
            memory: None,
            deadline: Instant::now() + Duration::from_secs(15),
        },
    );
    store.limiter(|host| &mut host.limits);
    store.set_fuel(fuel)?;
    store.set_epoch_deadline(1);
    store.epoch_deadline_callback(|context| {
        if Instant::now() >= context.data().deadline {
            return Err(wasmtime::Error::msg("Avatar decode exceeded 15 seconds"));
        }
        Ok(UpdateDeadline::Continue(1))
    });
    let memory = Memory::new(
        &mut store,
        MemoryType::new(512, Some(u32::try_from(HEAP / 65536)?)),
    )?;
    store.data_mut().memory = Some(memory);
    let mut linker = Linker::new(engine);
    linker.define(&store, "a", "a", memory)?;
    linker.func_wrap(
        "a",
        "b",
        |_fd: i32, _iov: i32, _count: i32, _written: i32| -> i32 { 8 },
    )?;
    linker.func_wrap("a", "d", |_fd: i32| -> i32 { 8 })?;
    linker.func_wrap("a", "e", || -> wasmtime::Result<()> {
        Err(wasmtime::Error::msg("Avatar codec aborted"))
    })?;
    linker.func_wrap("a", "f", |_code: i32| -> wasmtime::Result<()> {
        Err(wasmtime::Error::msg("Avatar codec exited"))
    })?;
    linker.func_wrap(
        "a",
        "c",
        |mut caller: Caller<'_, Host>, requested: u32| -> i32 {
            let Some(memory) = caller.data().memory else {
                return 0;
            };
            let requested = requested as usize;
            if requested > HEAP {
                return 0;
            }
            let current = memory.data_size(&caller);
            if requested <= current {
                return 1;
            }
            i32::from(
                memory
                    .grow(&mut caller, (requested - current).div_ceil(65536) as u64)
                    .is_ok(),
            )
        },
    )?;
    let instance = linker.instantiate(&mut store, module)?;
    instance
        .get_typed_func::<(), ()>(&mut store, "g")?
        .call(&mut store, ())?;
    Ok((store, instance, memory))
}

fn decode_image(
    input: &[u8],
    (mut store, instance, memory): (Store<Host>, Instance, Memory),
) -> Result<ImageInfo> {
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
    let mut result = ImageInfo {
        width: info.call(&mut store, 0)?,
        height: info.call(&mut store, 1)?,
        bits: info.call(&mut store, 2)?,
        frames: 0,
        loops: info.call(&mut store, 6)?,
        ticks_per_second_numerator: 1,
        ticks_per_second_denominator: 1,
        durations: Vec::with_capacity(1000),
    };
    ensure!(
        result.width > 0
            && result.width <= 1024
            && result.height > 0
            && result.height <= 1024
            && matches!(result.bits, 8 | 16),
        "Invalid avatar dimensions or precision"
    );
    let animated = info.call(&mut store, 3)? != 0;
    if animated {
        result.ticks_per_second_numerator = info.call(&mut store, 4)?;
        result.ticks_per_second_denominator = info.call(&mut store, 5)?;
        ensure!(
            result.ticks_per_second_numerator > 0 && result.ticks_per_second_denominator > 0,
            "Invalid avatar timebase"
        );
    } else {
        result.loops = 1;
    }
    let canvas = result.width * result.height;
    let size = canvas * 4 * (result.bits / 8);
    let pixel_pointer = alloc.call(&mut store, size)?;
    ensure!(pixel_pointer != 0, "Avatar canvas allocation failed");
    let mut header = false;
    let mut final_frame = false;
    let mut complete = false;
    let mut ticks = 0_u64;
    loop {
        ensure!(
            Instant::now() < store.data().deadline,
            "Avatar decode deadline exceeded"
        );
        match next.call(&mut store, ())? {
            0 => {
                ensure!(complete && final_frame && !header, "Incomplete avatar");
                break;
            }
            2 => {
                ensure!(
                    !header && !final_frame && result.frames < 1000,
                    "Invalid avatar frame order/count"
                );
                result.frames += 1;
                ensure!(
                    u64::from(canvas) * u64::from(result.frames) <= 100_000_000,
                    "Avatar cumulative pixel limit"
                );
                let duration = info.call(&mut store, 7)?;
                ticks += u64::from(duration);
                ensure!(
                    !animated
                        || u128::from(ticks) * u128::from(result.ticks_per_second_denominator)
                            <= 120 * u128::from(result.ticks_per_second_numerator),
                    "Avatar cycle exceeds 120 seconds"
                );
                result.durations.push(duration);
                final_frame = info.call(&mut store, 9)? == 1;
                header = true;
            }
            3 => {
                ensure!(
                    header && pixels.call(&mut store, (pixel_pointer, size))? >= 0,
                    "Invalid avatar canvas"
                );
            }
            4 => {
                ensure!(header, "Avatar frame without header");
                header = false;
                complete = true;
            }
            _ => bail!("JPEG XL decode rejected image"),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
