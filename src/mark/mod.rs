//! Pixel and sample marks with a public decoder. A mark here is confirmable
//! without a key: its presence is decided by a declared rule and its removal
//! is proven by re-running the decoder.

#[cfg(feature = "image")]
pub mod dwtdct;
