pub trait Progress {
    fn inc(&self, i: u64);
    fn finish(&self);
}

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

#[derive(Default)]
pub struct ProgressBar {
    bar: Option<Box<dyn Progress>>,
}

impl Progress for ProgressBar {
    fn inc(&self, i: u64) {
        self.bar.as_ref().map(|b| b.inc(i));
    }

    fn finish(&self) {
        self.bar.as_ref().map(|p| p.finish());
        if self.bar.is_some() {
            // restore logging
            log::set_max_level(log::LevelFilter::Info);
        }
    }
}

impl ProgressBar {
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
                .template("{bar:60.cyan/cyan} {msg} {pos}/{len} [{elapsed}]"),
        );
        bar.set_message(message.to_owned());
        // temporarily disable logging to not overwrite the bar
        log::set_max_level(log::LevelFilter::Off);
        ProgressBar {
            bar: Some(Box::new(bar)),
        }
    }

    fn logbar(len: u64, message: &str) -> Self {
        let style = logbar::Style::new().indicator('█');
        eprintln!("{}", message);
        let bar = logbar::ProgressBar::with_style(len as usize, style);
        // temporarily disable logging to not overwrite the bar
        log::set_max_level(log::LevelFilter::Off);
        ProgressBar {
            bar: Some(Box::new(bar)),
        }
    }
}
