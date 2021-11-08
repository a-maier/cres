pub use crate::{
    cres::{Cres, CresBuilder},
    converter::HepMCConverter,
    unweight::Unweighter,
};

pub type HepMCReader = crate::hepmc2::reader::CombinedReader;
pub type HepMCWriter<T> = crate::hepmc2::writer::Writer<T>;
pub type HepMCWriterBuilder<T> = crate::hepmc2::writer::WriterBuilder<T>;
