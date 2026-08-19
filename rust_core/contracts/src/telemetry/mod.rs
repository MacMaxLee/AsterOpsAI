//! Wire types for host telemetry (unit U1). See docs/SRS.md §6-14,
//! docs/TRS.md §6-7.

pub mod cpu;
pub mod device;
pub mod history;
pub mod memory;
pub mod metric_value;
pub mod network;
pub mod pressure;
pub mod process;
pub mod storage;
pub mod system_status;

pub use cpu::CpuSnapshot;
pub use device::{DeviceInfo, DeviceKind, DeviceSnapshot};
pub use history::{
    CpuHistoryPoint, HistoryResponse, HistoryStat, HistoryTier, MemoryHistoryPoint,
    NetworkHistoryPoint, StorageHistoryPoint,
};
pub use memory::{MemorySnapshot, NumaNodeMemory};
pub use metric_value::MetricValue;
pub use network::{NetworkInterfaceInfo, NetworkSnapshot};
pub use pressure::{CpuPressure, MemoryPressure};
pub use process::{ProcessCategory, ProcessInfo, ProcessSnapshot};
pub use storage::{StorageSnapshot, VolumeInfo};
pub use system_status::SystemStatusResponse;
