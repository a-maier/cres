pub mod reader;
pub mod writer;

/// Read events from one or more inputs in HepMC 2 format
pub use reader::FileReader;
pub use writer::{Writer, WriterBuilder};
