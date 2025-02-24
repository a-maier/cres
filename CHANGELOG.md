# Changelog

## 0.9.1

### Added

- Document minimal supported Rust version (MSRV).

## 0.9.0

### Added

- Accessor functions for neighbour search cached
- (C API) Conversion form `EventView` to `Event`

### Fixed

- Central weight updating in HepMC records. Usually this would only
  affect the last digit in the updated weight, except when it was
  zero.

## 0.8.0

### Added

- When generating shell completions, try to guess the shell if the
  argument is omitted.
- `noisy_float` and `particle_id` types in public interfaces are now
  re-exported
- [C API] Make `TypeSet::view` to convert from `TypeSet` to
  ``TypeSetView` public.

### Changed

- Extended output for identified event categories
- Events without outgoing particles are excluded from resampling
- Updated dependencies

### Fixed

- Various fixes in documentation and error messages

## 0.7.1

### Fixed

- Isolated photons were never found

## 0.7.0

### Added

- This changelog
- Photon isolation
- Serialisation support
- Pre-partitioning for better resampling of multiple samples
- Log output for identified event categories

### Changed

- Major change in event output: instead of writing a single file in an
  arbitrary format, for each input event file a corresponding output
  file in the same format will be written to an output directory set
  by the command line options
- Revamped event I/O, allowing parallelisation
- Resampled event weights are now set to the cell mean

### Removed

- `dumpcells` option

### Fixed

- Parsing of compression level

## 0.6.1

### Changed

- Updated documentation

### Fixed

- Don't output dummy values for git branch and hash

## 0.6.0

### Added

- Support for LHEF output
- Multiweight resampling
- Print enabled features when running
- Experimental support for the STRIPPER XML format

### Changed

- Resampled event weights are now set to the cell mean
- Revamped event I/O again
- Full parallelisation of vantage-point tree search
- Much faster jet clustering for high multiplicities
- (C API) don't build the bindings by default

### Removed

- Pre-partitioning

## 0.5.0

### Added

- Support for LHEF input
- Misc. documentation
- Negative weight statistics

### Changed

- Include (anti-)hadrons in jet clustering
- Include muons in lepton clustering

### Removed

- Option for configuration file, which was never implemented

### Fixed

- Ensure that momenta are in GeV
- Line breaks in readme

## 0.4.5

### Added

- (CI) deploy shell completion

## 0.4.4

## 0.4.3

### Added

- (C API) option to set number of partitions


## 0.4.2

### Fixed

- Compilation error

## 0.4.1

### Fixed

- Parser for number of partitions

## 0.4.0

### Added

- Vantage-point tree search with optional pre-partitioning
- Option to choose neighbour search algorithm
- Option to set number of threads
- Option to set output file format
- Support for lepton dressing (via clustering)
- Support for ROOT ntuple event files
- Version command line flag
- Progress bar for event reading if the number of events is known

### Changed

- Massive changes to event reading
- Default nearest-neighbour search now uses vantage-point trees
- Relax requirements on distance function
- Leave cross section entries in event records unchanged
- Slight simplification in example
- Better algorithm (Hungarian) for high-multiplicity distance calculations
- Reduced memory usage
- `Event` `id` is public again

### Fixed

- Option inconsistency between `cres` and `cres-partition`

## 0.3.3

### Added

- (CI) dynamic library on apple-darwin

### Changed

- Buffer file output by default
- Updated example documentation
- (CI) migrate to github actions

## 0.3.2-ci

## 0.3.2

### Added

- (CI) deploy library

### Fixed

- Regression: distance measure assumed wrong order of particle types

## 0.3.1

### Fixed

- Only try to get git info if building in a git repository

## 0.3.0

### Added

- C API
- Examples and lots of documentation

### Changed

- Use rust 2021
- Default seed selection
- Better reader error messages
- `Event` `id` and `outgoing_by_pid` members are no longer public

## 0.2.3

### Added

- Version information

### Fixed

- Line breaks in readme

## 0.2.2

### Fixed

- Formula in readme

## 0.2.1

### Fixed

- Syntax in readme
- Distance calculation

## 0.2.0-ci

## 0.2.0

### Added

- A readme
- License information (GPL-3.0 or later)
- Documentation
- (CI) Continuous Integration and Deployment

## 0.1.0

### Added

- Resampling
- HepMC input & output
- Automatic input decompression
- Output compression
- Command line options
- Progress bar
- Optional unweighting

### Changed

- Better error handling

### Removed

- Custom input event format

### Fixed

- Name of installed binary
- Computation of distance function
- Cell radius update
- Read _all_ input event files, not just the first one
