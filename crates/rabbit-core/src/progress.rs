//! Progress-event plumbing for the install and uninstall pipelines.
//!
//! The setup pipeline is normally silent until its final [`SetupReport`] is
//! returned: callers see a finished result, not the per-package boundaries
//! along the way. UIs that want a live progress bar need finer-grained
//! signals — which package is being downloaded right now, how many bytes
//! have arrived, when each install phase finishes. [`ProgressReporter`]
//! threads an opt-in callback through the pipeline so callers can wire one
//! in without forcing the existing API (CLI, tests, embedded callers) to
//! pass a sentinel through every function in the chain.
//!
//! Threading: the callback fires on whatever thread is currently driving
//! the pipeline — download pool workers and the operation's install thread
//! for the wxdragon UI, the calling thread for the CLI — and MAY be invoked
//! from several threads concurrently (downloads run on a small concurrent
//! pool). The closure must therefore be `Send + Sync` and tolerate
//! concurrent invocation. UIs that need to touch widgets cross-thread are
//! expected to forward events into their own UI thread (e.g. via
//! `wxdragon::call_after`).
//!
//! Ordering guarantees are PER PACKAGE only: a package's
//! `DownloadStarted` → `DownloadProgress`… → `DownloadCompleted` →
//! `InstallStarted` → `InstallCompleted` sequence always arrives in order,
//! but events of DIFFERENT packages interleave freely — several packages
//! can be mid-download at once, and one package's install runs while
//! others still download. Consumers must key any cross-event state by
//! `package_id`.
//!
//! Event-rate guarantees: stage events (`Started` / `Completed`) fire at
//! most a handful of times per package; the [`ProgressEvent::Download`]
//! byte-progress events are throttled by the download loop to roughly one
//! per 200 ms or per 256 KiB, whichever is rarer — per active download, so
//! N concurrent downloads emit up to ~5·N events per second in aggregate.
//!
//! [`SetupReport`]: crate::setup::SetupReport

use std::sync::Arc;

/// A single progress signal emitted by the install or uninstall pipeline.
/// The variants cover the three observable phases of an install (download,
/// install, configuration) plus per-byte progress within a download, and the
/// removal of each package in an uninstall.
///
/// Events carry only the package / step identity, not an "n of N" count.
/// UIs already know how many packages they queued (from the install plan)
/// and can derive overall progress by tracking completed-stage transitions
/// themselves. Keeping the events identity-only also means the pipeline
/// doesn't have to recompute totals when skipped-current packages exit
/// the work set early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressEvent {
    /// A package's artifact download is about to begin. `bytes_total` is
    /// `None` when the upstream didn't send `Content-Length` (or when the
    /// cached file is reused — in which case `DownloadCompleted` follows
    /// immediately without any [`ProgressEvent::DownloadProgress`] in
    /// between).
    DownloadStarted {
        package_id: String,
        bytes_total: Option<u64>,
    },
    /// Bytes-downloaded checkpoint within an in-flight download. Fires
    /// roughly every 200 ms / 256 KiB during a fresh download; never
    /// fires when the cached artifact is reused.
    DownloadProgress {
        package_id: String,
        bytes_downloaded: u64,
        bytes_total: Option<u64>,
    },
    /// The package's artifact is on disk (either freshly downloaded or
    /// reused from cache). This package's `InstallStarted` follows once the
    /// sequential install lane reaches it — with OTHER packages' download
    /// and install events possibly interleaved in between.
    DownloadCompleted { package_id: String },
    /// The on-disk install step is starting (copying the extension binary
    /// into UserPlugins, running the unattended vendor installer, mounting
    /// a dmg, extracting an archive, …).
    InstallStarted { package_id: String },
    /// The install step finished for this package, either successfully or
    /// by being staged as deferred. Errors don't fire this event — they
    /// short-circuit the pipeline with a [`crate::error::RabbitError`].
    InstallCompleted { package_id: String },
    /// A [`crate::configuration::ConfigurationStep`] is about to apply.
    /// `step_id` matches `ConfigurationStep::id`.
    ConfigurationStarted { step_id: String },
    /// A configuration step finished. As with `InstallCompleted`, errors
    /// short-circuit instead of firing this variant.
    ConfigurationCompleted { step_id: String },
    /// An uninstall is about to remove this package. Uninstalls run one
    /// package at a time and download nothing, so their events never
    /// interleave.
    RemoveStarted { package_id: String },
    /// The uninstall is done with this package. It fires after every
    /// `RemoveStarted`, also when the removal failed or a check stopped it,
    /// so a progress page never waits on a package that is over.
    /// `removed` says whether the package's files are gone; the report says
    /// the rest.
    RemoveCompleted { package_id: String, removed: bool },
}

/// A `Send + Sync` handle to the user-supplied progress callback. Cheap
/// to clone (it's an `Arc` under the hood), so pipeline code can stash
/// copies in worker structs or pass it by value where `&` would be
/// awkward.
///
/// Construct with [`ProgressReporter::new`] for a real callback, or
/// [`ProgressReporter::noop`] for callers that just want the silent
/// default — the public no-progress entry points use the latter
/// internally so they don't need to type the closure boilerplate.
#[derive(Clone)]
pub struct ProgressReporter {
    callback: Arc<dyn Fn(ProgressEvent) + Send + Sync>,
}

impl ProgressReporter {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(ProgressEvent) + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(callback),
        }
    }

    /// A reporter that drops every event. Use for the no-progress entry
    /// points and tests that don't care about the event stream.
    pub fn noop() -> Self {
        Self {
            callback: Arc::new(|_| {}),
        }
    }

    pub fn report(&self, event: ProgressEvent) {
        (self.callback)(event);
    }
}

impl std::fmt::Debug for ProgressReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressReporter").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn noop_reporter_swallows_events() {
        let reporter = ProgressReporter::noop();
        reporter.report(ProgressEvent::DownloadCompleted {
            package_id: "reaper".to_string(),
        });
    }

    #[test]
    fn custom_reporter_receives_events_in_order() {
        let captured: Arc<Mutex<Vec<ProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_closure = Arc::clone(&captured);
        let reporter = ProgressReporter::new(move |event| {
            captured_for_closure.lock().unwrap().push(event);
        });

        reporter.report(ProgressEvent::DownloadStarted {
            package_id: "osara".to_string(),
            bytes_total: Some(1234),
        });
        reporter.report(ProgressEvent::InstallCompleted {
            package_id: "osara".to_string(),
        });

        let captured = captured.lock().unwrap();
        assert_eq!(captured.len(), 2);
        matches!(captured[0], ProgressEvent::DownloadStarted { .. });
        matches!(captured[1], ProgressEvent::InstallCompleted { .. });
    }
}
