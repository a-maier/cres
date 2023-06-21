use std::{collections::HashMap, path::PathBuf, io::{BufReader, Seek}};

use noisy_float::prelude::*;
use stripper_xml::{SubEvent, Eventrecord};

use crate::{file::File, traits::{Rewind, TryClone}, reader::{RewindError, EventReadError, CreateError}, stripper_xml::{extract_xml_info, XMLTag}, auto_decompress::auto_decompress};

#[derive(Debug)]
pub struct Reader {
    file: File,
    events: Vec<SubEvent>,
    scale: N64,
    eof_reached: bool,
}

impl Reader {
    pub fn new(
        path: PathBuf,
        scaling: &HashMap<String, N64>,
    ) -> Result<Self, CreateError> {
        let file = File::open(&path)?;
        let input = file.try_clone()?;
        let mut input = auto_decompress(BufReader::new(input));
        let buf = input.fill_buf()?;
        let tag = extract_xml_info(path.as_path(), buf)?;
        let XMLTag::Eventrecord { name, .. } = tag else {
            panic!("Can no longer find Eventrecord")
        };
        let Some(scale) = scaling.get(&name).copied() else {
            panic!("No scaling factor")
        };
        Ok(Self {
            file,
            events: Vec::new(),
            scale,
            eof_reached: false,
        })
    }

    fn load_events(&mut self) -> Result<(), EventReadError> {
        let input = self.file.try_clone()?;
        let input = auto_decompress(BufReader::new(input));
        let record: Eventrecord = quick_xml::de::from_reader(input)?;
        self.eof_reached = true;
        self.events = record.events.into_iter()
            .flat_map(|e| e.subevents.into_iter())
            .collect();
        self.events.reverse();
        Ok(())
    }
}


impl Rewind for Reader {
    type Error = RewindError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        self.file.rewind()?;
        self.eof_reached = false;
        Ok(())
    }
}

impl Iterator for Reader {
    type Item = Result<hepmc2::Event, EventReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.eof_reached {
            if let Err(err) = self.load_events() {
                return Some(Err(err));
            }
        }
        debug_assert!(self.eof_reached);
        self.events.pop().map(|mut e| {
            e.weight *= f64::from(self.scale);
            Ok((&e).into())
        })
    }
}
