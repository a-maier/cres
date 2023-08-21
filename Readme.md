cres
====

This crate implements the cell resampling algorithm for the
elimination of negative weights in Monte Carlo collider event
samples. The algorithm is described in

Unbiased Elimination of Negative Weights in Monte Carlo Samples\
J. Andersen, A. Maier\
[arXiv:2109.07851](https://arxiv.org/abs/2109.07851)

Efficient negative-weight elimination in large high-multiplicity Monte Carlo event samples\
Jeppe R. Andersen, Andreas Maier, Daniel Maître\
[arXiv:2303.15246](https://arxiv.org/abs/2303.15246)


Installation
------------

If [Rust and Cargo](https://www.rust-lang.org/) are installed on your
system, run

    cargo install cres

Precompiled executables are available on
[hepforge](https://cres.hepforge.org/).

To install the development version, clone this repository and run

    cargo install --path path/to/local/cres/repository

Check the [Features section](#Features) for more options.

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

    cres -a JETALGO -R JETR --jetpt JETPT --max-cell-size R -o OUT.HEPMC2 IN.HEPMC2

This takes a file `IN.HEPMC2` (or several files) in hepmc2 or Les
Houches Event format with mixed-weight events and produces a file
`OUT.HEPMC2` with a smaller contribution from negative weights. The
input file can be compressed with bzip2, gzip, zstd, or lz4. The input
format is detected automatically, the output format can be set with
the `--outformat` option and defaults to hepmc2.

We recommend to set the jet algorithm `JETALGO`, jet radius `JETR`,
and minimum jet transverse momentum `JETPT` to the same values that
were used to generate the input events. The supported jet algorithms
are anti-kt, kt, and Cambridge-Aachen. When including QED corrections,
for instance through a shower, one should also set
`--leptonalgorithm`, `--leptonradius`, and `--leptonpt`.

Setting a maximum cell radius `R` is optional, but highly
recommended. Lower values lead to much faster resampling and smaller
smearing effects. Larger values eliminate a larger fraction of
negative weights. It is recommended to start with a value between 1
and 10 and adjust as needed.

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

- `--leptonalgorithm`, `--leptonradius`, `--leptonpt` enable
  clustering for leptons and photons. These options should be set
  whenever QED corrections are included, for example through
  showering.

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

## Features

To install cres with additional features, add `--features name1,name2`
to your installation command. Default features don't have to be added
manually. To disable them, add the `--no-default-features` flag.

### Default features

- `multiweight`: Enables the `--weights` option for treating multiple
  weights in one run. If you only want to consider a single weight you
  can disable this feature to save some memory and computing time.

- `lhef`: Support for reading and writing files in the Les
  Houches Event format.

### Non-default features

- `ntuple`: Support for reading and writing [ROOT ntuple
  files](https://arxiv.org/abs/1310.7439). This requires a recent
  version of `libclang` and a [ROOT](https://root.cern.ch/)
  installation with `root-config` in the executable path.

- `stripper-xml`: Experimental support for the XML format used by
  [STRIPPER](https://arxiv.org/abs/1005.0274).

- `capi`: Enables the C API for using `cres` as a C library. For
  examples, see the
  [examples](https://github.com/a-maier/cres/tree/master/examples)
  subdirectory. The API is limited and only available on unixoid
  platforms. It will be extended on request.

Use as a library
----------------

For full flexibility like custom distance functions `cres` can be used
as a library. For examples, see the
[examples](https://github.com/a-maier/cres/tree/master/examples)
subdirectory. The API is documented on
[docs.rs](https://docs.rs/crate/cres/).
