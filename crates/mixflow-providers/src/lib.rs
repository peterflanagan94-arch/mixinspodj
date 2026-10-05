//! GET MUSIC acquisition providers. Every source of new tracks goes through
//! the `MusicProvider` trait so the rest of the app can stay offline-first
//! and so no adapter here ever touches DRM circumvention, stream ripping,
//! or terms-of-service-violating scraping — only legitimate local import,
//! purchase-link hand-off, and officially supported APIs.
//!
//! Not implemented yet — out of scope until after MVP 0.1 (see
//! PROJECT_STATUS.md). Planned first adapters: local file import,
//! "already own this" marking, and a purchase-link opener.

pub trait MusicProvider {
    fn name(&self) -> &str;
}
