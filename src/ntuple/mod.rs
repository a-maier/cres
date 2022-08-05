#![allow(dead_code)]
mod ntuplereader;

pub mod reader;
pub mod converter;
pub mod writer;

pub type Reader = reader::Reader;
pub type Converter = converter::NTupleConverter;
pub use writer::Writer;
