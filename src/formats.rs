/// Supported event file formats
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FileFormat {
    /// HepMC2 format, also known as `IO_GenEvent`
    #[default]
    HepMC2,
    #[cfg(feature = "lhef")]
    /// Les Houches Event Format
    Lhef,
    #[cfg(feature = "ntuple")]
    /// BlackHat ntuples
    BlackHatNtuple,
    #[cfg(feature = "stripper-xml")]
    /// XML format used by STRIPPER
    StripperXml,
}
