/* cell resampling with custom distance using C
 *
 * see `cres.c` for instructions on running examples
 */

#include "cres.h"

#include <math.h>
#include <stdio.h>

/* user-defined distance function
 *
 * note that this function has be thread-safe and must never return NaN!
 *
 * this function is just for demonstration
 * and doesn't make much sense physically
 */
double my_distance(
  void* data,              /* user-defined data */
  EventView const * ev1,
  EventView const * ev2
) {
  /* `data` can be used to pass our own parameters */
  const double E_fact = * (const double*) data;

  double dist = 0.;

  /* iterate over particle types
   *
   * for simplicity, we only compare events that have the same particle types
   * and the same number of particles for each type
   */
  if(ev1->n_type_sets != ev2->n_type_sets) return INFINITY;
  for(uintptr_t t = 0; t < ev1->n_type_sets; ++t) {
    if(
      (ev1->type_sets[t].pid != ev2->type_sets[t].pid)
      || (ev1->type_sets[t].n_momenta != ev2->type_sets[t].n_momenta)
    ) return INFINITY;

    for(uintptr_t i = 0; i < ev1->type_sets[t].n_momenta; ++i) {
      /* use d(p1,p2) = E_fact * |E1 - E2| + |p1_x - p2_x| */
      const double* p1 = ev1->type_sets[t].momenta[i];
      const double* p2 = ev2->type_sets[t].momenta[i];
      dist += E_fact * fabs(p1[0] - p2[0]) + fabs(p1[1] - p2[1]);
    }
  }
  return dist;
}

int main(int argc, char** argv) {
  if(argc < 3) {
    return 1;
  }

  int32_t res = cres_logger_from_env("CRES_LOG");
  if(res != 0) cres_print_last_err();

  Opt opt;
  opt.n_infiles = argc - 2;
  opt.infiles = argv + 1;
  opt.outfile = argv[argc - 1];

  opt.jet_def.algorithm = AntiKt;
  opt.jet_def.radius = 0.4;
  opt.jet_def.min_pt = 30.;
  opt.neighbour_search = Tree;
  opt.max_cell_size = INFINITY;

  /* custom distance function with `E_fact` as extra data */
  double E_fact = 0.5;
  DistanceFn dist = {
    .fun = my_distance,
    .data = &E_fact
  };
  opt.distance = &dist;

  /* build and run the resampler */
  res = cres_run(&opt);
  if(res != 0) cres_print_last_err();
  return res;
}
