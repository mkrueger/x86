#[macro_use]
mod dbg;

#[cfg(not(target_arch = "wasm32"))]
pub use dbg::set_debug_log_handler;

#[macro_use]
mod paging;

pub mod cpu;
pub mod native_devices;
pub mod native_runtime;

pub mod js_api;
pub mod profiler;

#[cfg(not(feature = "native-interpreter"))]
mod analysis;
#[cfg(not(feature = "native-interpreter"))]
mod codegen;
mod config;
#[cfg(not(feature = "native-interpreter"))]
mod control_flow;
#[cfg_attr(feature = "native-interpreter", allow(dead_code))]
mod cpu_context;
mod gen;
#[cfg(feature = "native-interpreter")]
#[allow(dead_code)]
#[path = "jit_native.rs"]
mod jit;
#[cfg(not(feature = "native-interpreter"))]
mod jit;
#[cfg(not(feature = "native-interpreter"))]
mod jit_instructions;
#[cfg(not(feature = "native-interpreter"))]
mod leb;
#[cfg_attr(feature = "native-interpreter", allow(dead_code, unused_imports))]
mod modrm;
#[cfg(feature = "native-interpreter")]
#[path = "opstats_native.rs"]
mod opstats;
#[cfg(not(feature = "native-interpreter"))]
mod opstats;
mod page;
#[cfg_attr(feature = "native-interpreter", allow(dead_code))]
mod prefix;
#[cfg_attr(feature = "native-interpreter", allow(dead_code))]
mod regs;
mod softfloat;
mod state_flags;
#[cfg(not(feature = "native-interpreter"))]
mod wasmgen;
mod zstd;
