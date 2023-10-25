use std::fmt::Debug;
use std::path::{Path, PathBuf};

use noisy_float::prelude::*;
use particle_id::ParticleID;

use crate::event::{Weights, Event, EventBuilder};
use crate::storage::{EventFileReader, EventRecord, FileStorageError, Converter, ReadError, ErrorKind, CreateError};
use crate::traits::{Rewind, UpdateWeights};

/// Reader from a BlackHat ntuple file
#[derive(Debug)]
pub struct FileReader {
    reader: ntuple::Reader,
    source_path: PathBuf,
}

impl FileReader {
    /// Construct a reader from the given (potentially compressed) HepMC2 event file
    pub fn try_new(source_path: PathBuf) -> Result<Self, CreateError> {
        use CreateError::NTuple;
        let reader = ntuple::Reader::new(&source_path)
            .ok_or_else(|| NTuple(format!("Failed to create ntuple reader for {source_path:?}")))?;

        Ok(Self {reader, source_path})
    }

    fn read_raw(&mut self) -> Option<Result<ntuple::Event, ReadError>> {
        self.reader.next().map(|n| n.map_err(ReadError::from))
    }
}

impl EventFileReader for FileReader {
    fn path(&self) -> &Path {
        self.source_path.as_path()
    }

    fn header(&self) -> &[u8] {
        &[]
    }
}

impl Rewind for FileReader {
    type Error = CreateError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        *self.reader.nevent_mut() = 0;
        Ok(())
    }
}

impl Iterator for FileReader {
    type Item = Result<EventRecord, ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.next().map(|n| match n {
            Ok(event) => Ok(EventRecord::NTuple(Box::new(event))),
            Err(err) => Err(err.into())
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.reader.size_hint()
    }
}

/// Reader for a single ROOT ntuple event file
#[derive(Debug)]
pub struct FileStorage{
    reader: FileReader,
    writer: ntuple::Writer,
    sink_path: PathBuf,
    _weight_names: Vec<String>,
}

impl FileStorage {
    /// Storage backed by ROOT ntuple files with the given names
    pub fn try_new(
        source_path: PathBuf,
        sink_path: PathBuf,
        _weight_names: Vec<String>
    ) -> Result<Self, CreateError> {
        use CreateError::NTuple;
        let reader = FileReader::try_new(source_path)?;
        let writer = ntuple::Writer::new(&sink_path, "")
            .ok_or_else(|| NTuple(format!("Failed to create ntuple writer to {sink_path:?}")))?;
        Ok(Self{reader, writer, _weight_names, sink_path })
    }

    #[allow(clippy::wrong_self_convention)]
    fn into_storage_error<T, E: Into<ErrorKind>>(
        &self,
        res: Result<T, E>
    ) -> Result<T, FileStorageError> {
        res.map_err(|err| FileStorageError::new(
            self.reader.path().to_path_buf(),
            self.sink_path.clone(),
            err.into()
        ))
    }
}

impl Rewind for FileStorage {
    type Error = FileStorageError;

    fn rewind(&mut self) -> Result<(), Self::Error> {
        let res = self.reader.rewind();
        self.into_storage_error(res)
    }
}

impl Iterator for FileStorage {
    type Item = Result<EventRecord, FileStorageError>;

    fn next(&mut self) -> Option<Self::Item> {
       self.reader
            .next()
            .map(|r| self.into_storage_error(r))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.reader.size_hint()
    }
}

/// Converter for ROOT ntuple event records
pub trait NTupleConverter {
    /// Error converting ROOT ntuple event record
    type Error;

    /// Convert ROOT ntuple event record
    fn convert_ntuple(&self, record: ntuple::Event) -> Result<Event, Self::Error>;
}

impl NTupleConverter for Converter {
    type Error = ErrorKind;

    fn convert_ntuple(&self, record: ntuple::Event) -> Result<Event, Self::Error> {
        let nparticle = record.nparticle as usize;
        let mut event = EventBuilder::with_capacity(nparticle);
        event.add_weight(n64(record.weight));
        #[cfg(feature = "multiweight")]
        {
            if self.weight_names().iter().any(|w| w == "2") {
                event.add_weight(n64(record.weight2));
            }
            if self.weight_names().iter().any(|w| w == "ME") {
                event.add_weight(n64(record.me_weight));
            }
            if self.weight_names().iter().any(|w| w == "ME2") {
                event.add_weight(n64(record.me_weight2));
            }
            // TODO: user weights
        }
        for i in 0..nparticle {
            let id = ParticleID::new(record.pdg_code[i]);
            let e  = n64(record.energy[i] as f64);
            let px = n64(record.px[i] as f64);
            let py = n64(record.py[i] as f64);
            let pz = n64(record.pz[i] as f64);
            event.add_outgoing(id, [e, px, py, pz].into());
        }
        Ok(event.build())
    }
}

impl UpdateWeights for FileStorage {
    type Error = FileStorageError;

    fn update_all_weights(
        &mut self,
        weights: &[Weights]
    ) -> Result<usize, Self::Error> {
        self.rewind()?;
        let mut nevent = 0;
        while self.update_next_weights(&weights[nevent])? {
            nevent += 1;
        }
        Ok(nevent)
    }

    fn update_next_weights(
        &mut self,
        weights: &Weights
    ) -> Result<bool, Self::Error> {
        let Some(record) = self.reader.read_raw() else {
            return Ok(false)
        };
        let mut record = self.into_storage_error(record)?;

        let mut weights = weights.iter().copied();
        record.weight = weights.next().unwrap().into();

        #[cfg(feature = "multiweight")]
        {
            // beware: order here has to match `convert_ntuple`
            // TODO: user weights
            if self._weight_names.iter().any(|w| w == "2") {
                record.weight2 = weights.next().unwrap().into();
            }
            if self._weight_names.iter().any(|w| w == "ME") {
                record.me_weight = weights.next().unwrap().into()
            }
            if self._weight_names.iter().any(|w| w == "ME2") {
                record.me_weight2 = weights.next().unwrap().into()
            }
        }
        self.writer.write(&record).unwrap();
        Ok(true)
    }
}
