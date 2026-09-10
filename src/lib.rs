//! Platform-neutral LiteList domain model.
//!
//! The native Windows executable keeps its Win32 UI, while this library target
//! lets both Linux CI and Windows builds verify the shared document contract.
pub mod model;

#[cfg(windows)]
pub mod reminders;

#[cfg(windows)]
pub mod storage;
