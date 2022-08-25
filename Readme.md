cres
====

This crate implements the cell resampling algorithm for the
elimination of negative weights in Monte Carlo collider event
samples. The algorithm is described in

Unbiased Elimination of Negative Weights in Monte Carlo Samples\
J. Andersen, A. Maier\
[arXiv:2109.07851](https://arxiv.org/abs/2109.07851)

Installation
------------

If [Rust and Cargo](https://www.rust-lang.org/) are installed on your
system, run

    cargo install cres

Precompiled executables are available on
[hepforge](https://cres.hepforge.org/).

To generate shell command completion, run

    cres-generate-shell-completion SHELL

For bash and fish, command completion should work upon the next login.
For other shells, the completion code is written to standard
output. Consult your shell's documentation if you are unsure what to
do with it. To list the supported shells, run

    cres-generate-shell-completion --help

Usage
-----

The basic usage is

    cres -a JETALGO -R JETR --jetpt JETPT -o OUT.HEPMC2 IN.HEPMC2

This takes a file `IN.HEPMC2` in hepmc2 format with mixed-weight
events and produces a file `OUT.HEPMC2` where all event weights are
positive. The input file can be compressed with bzip2, gzip, zstd, or
lz4.

We recommend to set the jet algorithm `JETALGO`, jet radius
`JETR`, and minimum jet transverse momentum `JETPT` to the same values
that were used to generate the input events. The supported jet
algorithms are anti-kt, kt, and Cambridge-Aachen.

Options
-------

To see a full list of options with short descriptions run

    cres --help

The most important options are

- `--max-cell-size` can be used to limit the size of the generated
  cells. This ensures that weights are only transferred between events
  which are sufficiently similar. The downside is that not all
  negative event weights will be removed.

  Cell resampling is much faster with a small cell size limit. It is
  therefore recommended to start with a small value, for example 10,
  and gradually increase the value if too many negative weights are
  left.

- `--partitions` the initial sample is divided into the given number
  of partitions. The number of partitions has to be a power of two.
  On the one hand, values larger than one accelerate the
  nearest-neighbour search and are necessary to parallelise the
  default search strategy. On the other hand, nearest neighbours may
  be missed for cells close to partitions boundaries.

  Recommended values are 1 for slow, but exact resampling, or a number
  up to about twice the number of cores for faster approximate
  resampling. Increasing the number of partitions is better than
  increasing the number of runs with different event input files.

- `--ptweight` specifies how much transverse momenta affect distances
  between particles with momenta p and q according to the formula

      d(p, q) = \sqrt{ ptweight^2 (p_\perp - q_\perp)^2 + \sum (p_i - q_i)^2 }

- With `--minweight` events are also unweighted in addition to the
  resampling.  Events with weight `w < minweight` are discarded with
  probability `1-|w|/minweight` and reweighted to `sign(w) * minweight`
  otherwise. Finally, all event weights are rescaled to exactly
  preserve the original sum of weights. The seed for unweighting can
  be chosen with the `--seed` option.

There are too many options
--------------------------

To avoid cluttering the command line, options can be saved in an
argfile. Each line should contain exactly one option, and option name
and value have to be separated by '='. For example:

```
--jetalgorithm=anti-kt
--jetradius=0.4
--jetpt=30
```

The argfile can be used like this:

    cres @argfile -o OUT.HEPMC2 IN.HEPMC2

Environment variables
---------------------

The `CRES_LOG` environment variable allows fine-grained control over
the command line output. For example, to see the debugging output of
the jet clustering, set

    CRES_LOG=jetty=debug,cres=info

See the [`env_logger` crate](https://crates.io/crates/env_logger/) for a
comprehensive documentation.

By default, `cres` uses as many cores as possible. For small event
samples, limiting the number of threads can be faster. You can set the
number of threads with the `--threads` command line option or with the
`RAYON_NUM_THREADS` environment variable.

Support for ntuple files
------------------------

To read and write [ROOT ntuple files](https://arxiv.org/abs/1310.7439),
ROOT has to be installed and `root-config` has to be in the executable
path. To install `cres` with ntuple support, run

    cargo install cres --features ntuple

Input files are recognised automatically, the output file format can
be chosen with the `--outformat` flag.

Use as a library
----------------

For full flexibility like custom distance functions `cres` can be used
as a library from Rust and C. For examples, see the `examples`
subdirectory. The Rust API is documented on
[docs.rs](https://docs.rs/crate/cres/). The C API is still limited and
only available on unixoid platforms. It will be extended on request.
