/* cell resampling using C
 *
 * To run this example, first download the cres library from
 * https://github.com/a-maier/cres/releases
 *
 * Alternatively, compile the library running
 * ```
 * cargo build --release
 * ```
 * in the directory containing `Cargo.toml`. To compile with support for
 * ROOT ntuple files instead use
 * ```
 * cargo build --release --features=ntuple
 * ```
 * and similar for other features described in the Readme.
 *
 * Then copy the compiled libraries (`libcres.a` and `libcres.so` on
 * linux) and the generated header `cres.h` to a directory where
 * they can be found by your C compiler.
 *
 * Now compile the example, for example with
 * ```
 * cc -o cres examples/cres.c -lcres -lm
 * ```
 *
 * Finally, run with
 * ```
 * ./cres INFILES OUTDIR
 * ```
 */

#include "cres.h"

#include <math.h>
#include <stdio.h>

int main(int argc, char** argv) {
  if(argc < 3) {
    return 1;
  }

  /* initialise logger from environment variable
     (only if we want progress output)
  */
  int32_t res = cres_logger_from_env("CRES_LOG");
  if(res != 0) cres_print_last_err();

  Opt opt;
  /* all but the first and last command line arguments are the input files */
  opt.n_infiles = argc - 2;
  opt.infiles = argv + 1;
  /* and the last command line arguments is the output file */
  opt.outdir = argv[argc - 1];

  /* settings for jet clustering */
  opt.jet_def.algorithm = AntiKt;
  opt.jet_def.radius = 0.4;
  opt.jet_def.min_pt = 30.;
  /* maximum cell size, INFINITY means effectively unlimited */
  opt.max_cell_size = INFINITY;

  /* algorithm for finding nearest-neighbour events */
  opt.neighbour_search = Tree;

  /* distance function
   *
   * `NULL` means the standard distance function described in
   * https://arxiv.org/abs/2109.07851
   *
   * differences in transverse momentum are enhanced by τ = opt.ptweight
   *
   * see `user_distance.c` for an example of a user-defined distance
   */
  opt.distance = NULL;
  opt.ptweight = 0.;

  /* build and run the resampler */
  res = cres_run(&opt);
  if(res != 0) cres_print_last_err();
  return res;
}
