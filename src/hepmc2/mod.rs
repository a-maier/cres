pub mod converter;
pub mod reader;
pub mod writer;

/// Read events from one or more inputs in HepMC 2 format
pub type Reader<'a, R> = reader::CombinedReader<'a, R>;
pub use converter::{ClusteringConverter, Converter};
pub use writer::{Writer, WriterBuilder};
