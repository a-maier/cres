/* cell resampling using C
 *
 * To run this example, first compile the cres library running
 * ```
 * cargo build --release
 * ```
 * in the directory containing `Cargo.toml`.
 *
 * Then copy the compiled libraries (`libcres.a` and `libcres.so` on
 * linux) and the generated header `build/cres.h` to a directory where
 * they can be found by your C compiler.
 *
 * Now compile the example. For gcc use
 * ```
 * gcc -o cres examples/cres.c -lcres -lm
 * ```
 * or similar for clang.
 *
 * Finally, run with
 * ```
 * ./cres INFILES.hepmc OUTFILE.hepmc
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
  opt.outfile = argv[argc - 1];

  /* settings for jet clustering */
  opt.jet_def.algorithm = AntiKt;
  opt.jet_def.radius = 0.4;
  opt.jet_def.min_pt = 30.;
  /* factor between total cross section and sum of weights */
  opt.weight_norm = 1.;
  /* maximum cell size, INFIINITY means effectively unlimited */
  opt.max_cell_size = INFINITY;

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
