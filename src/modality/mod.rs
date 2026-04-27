//! Per-modality adapters that wrap the per-modality SDKs into uniform
//! [`crate::Record`] values.
//!
//! Each adapter is feature-gated so a build that doesn't need a
//! modality doesn't compile its SDK at all.

#[cfg(feature = "audio")]
pub mod audio;

#[cfg(feature = "image")]
pub mod image;

#[cfg(feature = "text")]
pub mod text;
