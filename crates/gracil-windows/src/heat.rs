//! How warm the computer is running (ADR 0043), read without drivers or administrator rights.
//!
//! - NVIDIA graphics cards: their temperature, the temperature at which the card starts slowing
//!   itself down, and whether it is slowing down because of heat, from `nvml.dll`, which comes
//!   with NVIDIA's driver. It is loaded from System32 only, and only if it is there.
//! - The processor: Windows' "% Performance Limit", which falls below its usual value when the
//!   processor is held back, for example because it is too hot. Its temperature itself can't be
//!   read without a driver.

use std::ffi::c_void;
use std::mem;
use std::ptr;

use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};
use windows_sys::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW,
    PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
};

/// One graphics card, as read just now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsHeat {
    /// Its temperature, in degrees Celsius.
    pub celsius: u32,
    /// The temperature at which the card starts slowing itself down, if it says.
    pub slowdown: Option<u32>,
    /// The card is slowing itself down because of heat right now.
    pub held_back: bool,
}

/// How warm the computer is running, as far as can be told.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Heat {
    /// Every NVIDIA graphics card. Empty for other cards, or without NVIDIA's driver.
    pub graphics: Vec<GraphicsHeat>,
    /// How much of its full speed the processor is allowed right now, in percent, if Windows
    /// says.
    pub processor_limit: Option<f64>,
}

/// Reads [`Heat`]. Keep one and read it from the same thread.
pub struct Thermometer {
    nvml: Option<Nvml>,
    processor: Option<Counter>,
}

impl Thermometer {
    /// Gets ready to read. Whatever can't be read is simply left out.
    pub fn new() -> Self {
        Self {
            nvml: Nvml::load(),
            processor: Counter::open(r"\Processor Information(_Total)\% Performance Limit"),
        }
    }

    /// Reads the computer now.
    pub fn read(&mut self) -> Heat {
        Heat {
            graphics: self.nvml.as_ref().map_or_else(Vec::new, Nvml::read),
            processor_limit: self.processor.as_ref().and_then(Counter::read),
        }
    }
}

impl Default for Thermometer {
    fn default() -> Self {
        Self::new()
    }
}

type Device = *mut c_void;
type Init = unsafe extern "C" fn() -> i32;
type Count = unsafe extern "C" fn(*mut u32) -> i32;
type HandleByIndex = unsafe extern "C" fn(u32, *mut Device) -> i32;
type Temperature = unsafe extern "C" fn(Device, i32, *mut u32) -> i32;
type Threshold = unsafe extern "C" fn(Device, i32, *mut u32) -> i32;
type Reasons = unsafe extern "C" fn(Device, *mut u64) -> i32;

const NVML_SUCCESS: i32 = 0;
const NVML_TEMPERATURE_GPU: i32 = 0;
const NVML_TEMPERATURE_THRESHOLD_SLOWDOWN: i32 = 1;
/// The card's own software, or its hardware, slowing it down because of heat.
const THERMAL_SLOWDOWN: u64 = 0x20 | 0x40;

/// The few NVML functions used, kept for as long as Gracil runs.
struct Nvml {
    count: Count,
    handle_by_index: HandleByIndex,
    temperature: Temperature,
    threshold: Option<Threshold>,
    reasons: Option<Reasons>,
}

impl Nvml {
    fn load() -> Option<Self> {
        let name: Vec<u16> = "nvml.dll".encode_utf16().chain([0]).collect();
        // SAFETY: the name is a NUL-terminated UTF-16 buffer that outlives the call; the file
        // handle must be null; System32 only, so no other copy can be picked up.
        #[allow(unsafe_code)]
        let library =
            unsafe { LoadLibraryExW(name.as_ptr(), ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        if library.is_null() {
            return None;
        }
        // SAFETY: each type matches the function's declaration in NVIDIA's nvml.h, whose
        // functions use the C calling convention, which is the only one on 64-bit Windows.
        #[allow(unsafe_code)]
        let nvml = unsafe {
            let init: Init = function(library, b"nvmlInit_v2\0")?;
            if init() != NVML_SUCCESS {
                return None;
            }
            Self {
                count: function(library, b"nvmlDeviceGetCount_v2\0")?,
                handle_by_index: function(library, b"nvmlDeviceGetHandleByIndex_v2\0")?,
                temperature: function(library, b"nvmlDeviceGetTemperature\0")?,
                threshold: function(library, b"nvmlDeviceGetTemperatureThreshold\0"),
                reasons: function(library, b"nvmlDeviceGetCurrentClocksEventReasons\0")
                    .or_else(|| function(library, b"nvmlDeviceGetCurrentClocksThrottleReasons\0")),
            }
        };
        Some(nvml)
    }

    fn read(&self) -> Vec<GraphicsHeat> {
        let mut count = 0;
        // SAFETY: NVML was initialized when loaded; every out-pointer is to a local that
        // outlives its call, and a device handle is only used after NVML handed it back.
        #[allow(unsafe_code)]
        unsafe {
            if (self.count)(&mut count) != NVML_SUCCESS {
                return Vec::new();
            }
            (0..count)
                .filter_map(|index| {
                    let mut device = ptr::null_mut();
                    if (self.handle_by_index)(index, &mut device) != NVML_SUCCESS {
                        return None;
                    }
                    let mut celsius = 0;
                    if (self.temperature)(device, NVML_TEMPERATURE_GPU, &mut celsius)
                        != NVML_SUCCESS
                    {
                        return None;
                    }
                    let slowdown = self.threshold.and_then(|threshold| {
                        let mut value = 0;
                        let read =
                            threshold(device, NVML_TEMPERATURE_THRESHOLD_SLOWDOWN, &mut value);
                        (read == NVML_SUCCESS && value > 0).then_some(value)
                    });
                    let held_back = self.reasons.is_some_and(|reasons| {
                        let mut value = 0;
                        reasons(device, &mut value) == NVML_SUCCESS && value & THERMAL_SLOWDOWN != 0
                    });
                    Some(GraphicsHeat {
                        celsius,
                        slowdown,
                        held_back,
                    })
                })
                .collect()
        }
    }
}

/// The function `name` (NUL-terminated) of `library`, as `F`.
///
/// # Safety
///
/// `F` must be a function pointer type matching the function's real declaration.
#[allow(unsafe_code)]
unsafe fn function<F: Copy>(library: HMODULE, name: &[u8]) -> Option<F> {
    // SAFETY: `name` is NUL-terminated, and `library` was loaded and is never freed.
    let address = unsafe { GetProcAddress(library, name.as_ptr()) }?;
    debug_assert_eq!(mem::size_of::<F>(), mem::size_of_val(&address));
    // SAFETY: both are function pointers of the same size; the caller vouches for the type.
    Some(unsafe { mem::transmute_copy(&address) })
}

/// One of Windows' performance counters, by its English name, so it works in every language.
struct Counter {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

impl Counter {
    fn open(path: &str) -> Option<Self> {
        let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut query = ptr::null_mut();
        // SAFETY: a null data source means this computer, live; `query` outlives the call.
        #[allow(unsafe_code)]
        if unsafe { PdhOpenQueryW(ptr::null(), 0, &mut query) } != 0 {
            return None;
        }
        let mut counter = ptr::null_mut();
        // SAFETY: `query` was just opened, `path` is NUL-terminated and outlives the call.
        #[allow(unsafe_code)]
        if unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) } != 0 {
            // SAFETY: `query` was opened above and is not used again.
            #[allow(unsafe_code)]
            unsafe {
                PdhCloseQuery(query);
            }
            return None;
        }
        Some(Self { query, counter })
    }

    fn read(&self) -> Option<f64> {
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: the query and counter stay open while `self` lives; `value` outlives the
        // call, and the type out-pointer may be null.
        #[allow(unsafe_code)]
        let read = unsafe {
            PdhCollectQueryData(self.query) == 0
                && PdhGetFormattedCounterValue(
                    self.counter,
                    PDH_FMT_DOUBLE,
                    ptr::null_mut(),
                    &mut value,
                ) == 0
        };
        // SAFETY: the value was asked for as a double.
        #[allow(unsafe_code)]
        let percent = unsafe { value.Anonymous.doubleValue };
        (read && value.CStatus == 0 && percent.is_finite()).then_some(percent)
    }
}

impl Drop for Counter {
    fn drop(&mut self) {
        // SAFETY: the query was opened in `open` and is closed only here, with its counter.
        #[allow(unsafe_code)]
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_without_failing() {
        let mut thermometer = Thermometer::new();
        let heat = thermometer.read();
        for card in &heat.graphics {
            assert!(card.celsius < 150);
        }
        if let Some(limit) = heat.processor_limit {
            assert!(limit >= 0.0);
        }
    }
}
