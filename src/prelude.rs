pub use crate::{
    cres::{Cres, CresBuilder},
    converter::HepMCConverter,
    unweight::{Unweighter, NO_UNWEIGHTING},
};

pub type HepMCReader<'a, R> = crate::hepmc2::reader::CombinedReader<'a, R>;
pub type HepMCWriter<T> = crate::hepmc2::writer::Writer<T>;
pub type HepMCWriterBuilder<T> = crate::hepmc2::writer::WriterBuilder<T>;
