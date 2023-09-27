/// Reader for input in HepMC 2 format
pub mod reader;
/// Writer to HepMC 2 format
pub mod writer;

pub use reader::{FileReader, HepMCError};
pub use writer::Writer;
