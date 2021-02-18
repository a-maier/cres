pub(crate) trait ProgressBar {
    fn inc(&self, i: u64);
    fn finish(&self);
}

impl ProgressBar for indicatif::ProgressBar {
    fn inc(&self, i: u64) {
        indicatif::ProgressBar::inc(&self, i)
    }

    fn finish(&self) {
        indicatif::ProgressBar::finish(&self)
    }
}

impl ProgressBar for logbar::ProgressBar {
    fn inc(&self, i: u64) {
        logbar::ProgressBar::inc(&self, i as usize)
    }

    fn finish(&self) {
        logbar::ProgressBar::finish(&self)
    }
}

pub(crate) fn get_progress_bar(
    len: u64,
    message: &str
) -> Option<Box<dyn ProgressBar>>
{
    if log::max_level().to_level() != Some(log::Level::Info) {
        return None;
    }
    if console::Term::stderr().features().is_attended() {
        let progress = indicatif::ProgressBar::new(len);
        progress.set_style(
            indicatif::ProgressStyle::default_bar().template(
                "{bar:60.cyan/cyan} {msg} {pos}/{len} [{elapsed}]"
            )
        );
        progress.set_message(message);
        Some(Box::new(progress))
    }
    else {
        let style = logbar::Style::new().indicator('█');
        eprintln!("{}", message);
        let progress = logbar::ProgressBar::with_style(len as usize, style);
        Some(Box::new(progress))
    }
}
