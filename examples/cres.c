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
  opt.infiles = argv + 1;
  opt.n_infiles = argc - 2;
  opt.outfile = argv[argc - 1];

  opt.ptweight = 0.;
  opt.jet_def.algorithm = AntiKt;
  opt.jet_def.radius = 0.4;
  opt.jet_def.min_pt = 30.;
  opt.weight_norm = 1.;
  double inf = INFINITY;
  opt.max_cell_size = &inf;

  opt.distance = NULL;

  res = cres_run(&opt);
  if(res != 0) cres_print_last_err();
  return res;
}
