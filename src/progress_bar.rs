pub use crate::traits::Progress;

impl Progress for indicatif::ProgressBar {
    fn inc(&self, i: u64) {
        indicatif::ProgressBar::inc(self, i)
    }

    fn finish(&self) {
        indicatif::ProgressBar::finish(self)
    }
}

impl Progress for logbar::ProgressBar {
    fn inc(&self, i: u64) {
        logbar::ProgressBar::inc(self, i as usize)
    }

    fn finish(&self) {
        logbar::ProgressBar::finish(self)
    }
}

/// Dummy progress indicator
pub struct NoProgress {}
impl Progress for NoProgress {
    fn inc(&self, _i: u64) {}

    fn finish(&self) {}
}

/// Don't show any progress indicator
pub const NO_PROGRESS: NoProgress = NoProgress {};

/// The default progress bar
///
/// The exact format is decided at run time depending on whether we are
/// writing to an interactive terminal or a non-interactive output.
pub struct ProgressBar {
    bar: Box<dyn Progress + Send + Sync>,
    logging_disabled: bool,
}

impl Default for ProgressBar {
    fn default() -> Self {
        Self { bar: Box::new(NO_PROGRESS), logging_disabled: false }
    }
}

impl Progress for ProgressBar {
    fn inc(&self, i: u64) {
        self.bar.inc(i);
    }

    fn finish(&self) {
        self.bar.finish();
        if self.logging_disabled {
            // restore logging
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}

impl ProgressBar {
    /// A new progress bar with the given maximum progress and message
    pub fn new(len: u64, message: &str) -> Self {
        if log::max_level().to_level() != Some(log::Level::Info) {
            ProgressBar::default()
        } else if console::Term::stderr().features().is_attended() {
            ProgressBar::indicatif(len, message)
        } else {
            ProgressBar::logbar(len, message)
        }
    }

    fn indicatif(len: u64, message: &str) -> Self {
        let bar = indicatif::ProgressBar::new(len);
        bar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{bar:60.cyan/cyan} {msg} {pos}/{len} [{elapsed}]")
                .unwrap(),
        );
        bar.set_message(message.to_owned());
        // temporarily disable logging to not overwrite the bar
        log::set_max_level(log::LevelFilter::Off);
        ProgressBar {
            bar: Box::new(bar),
            logging_disabled: true,
        }
    }

    fn logbar(len: u64, message: &str) -> Self {
        let style = logbar::Style::new().indicator('█');
        eprintln!("{}", message);
        let bar = logbar::ProgressBar::with_style(len as usize, style);
        // temporarily disable logging to not overwrite the bar
        log::set_max_level(log::LevelFilter::Off);
        ProgressBar {
            bar: Box::new(bar),
            logging_disabled: true,
        }
    }
}
