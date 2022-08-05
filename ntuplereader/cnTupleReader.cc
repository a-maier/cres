#include "ntuplereader/nTupleReader.h"

// TODO: catch exceptions & return an error type

extern "C" {
#include "cnTupleReader.h"

nTupleReader *ntuple_reader_new() { return new nTupleReader; }
nTupleReader *ntuple_reader_from_tree(char const *treeName) {
  return new nTupleReader(treeName);
}

bool next_entry(nTupleReader *r) { return r->nextEntry(); }

void set_pdf(nTupleReader *r, char const *name) { return r->setPDF(name); }

void set_pdf_member(nTupleReader *r, int member) {
  return r->setPDFmember(member);
}
int get_id(nTupleReader *r) { return r->getID(); }
int get_particle_number(nTupleReader *r) { return r->getParticleNumber(); }
double get_energy(nTupleReader *r, int i) { return r->getEnergy(i); };
double get_x(nTupleReader *r, int i) { return r->getX(i); }

double get_y(nTupleReader *r, int i) { return r->getY(i); }

double get_z(nTupleReader *r, int i) { return r->getZ(i); }

int get_pdg_code(nTupleReader *r, int i) { return r->getPDGcode(i); }

double get_x1(nTupleReader *r) { return r->getX1(); }

double get_x2(nTupleReader *r) { return r->getX2(); }

double get_id1(nTupleReader *r) { return r->getId1(); }

double get_id2(nTupleReader *r) { return r->getId2(); }

short get_alphas_power(nTupleReader *r) { return r->getAlphasPower(); }

double get_renormalization_scale(nTupleReader *r) {
  return r->getRenormalizationScale();
}

double get_factorization_scale(nTupleReader *r) {
  return r->getFactorizationScale();
}

double get_weight(nTupleReader *r) { return r->getWeight(); }

double get_weight2(nTupleReader *r) { return r->getWeight2(); }

double get_me_weight(nTupleReader *r) { return r->getMEWeight(); }

double get_me_weight2(nTupleReader *r) { return r->getMEWeight2(); }

char get_type(nTupleReader *r) { return r->getType(); }

double compute_weight(
  nTupleReader *r, double newFactorizationScale,
  double newRenormalizationScale
) {
  return r->computeWeight(newFactorizationScale, newRenormalizationScale);
}

double compute_weight2(
  nTupleReader *r, double newFactorizationScale,
  double newRenormalizationScale
) {
  return r->computeWeight2(newFactorizationScale, newRenormalizationScale);
}

void set_pp(nTupleReader *r) { return r->setPP(); }

void set_ppbar(nTupleReader *r) { return r->setPPbar(); }


void drop_ntuple_reader(nTupleReader *r) { delete r; }

void add_file(nTupleReader *r, char const *filename) {
  return r->addFile(filename);
}

// void set_cms_energy(nTupleReader *r, double CMS_energy) {
//   return r->setCMSEnergy(CMS_energy);
// }
// void set_collider_type(nTupleReader *r, ColliderType ct) {
//   return r->setColliderType(static_cast<colliderType>(ct));
// }
void reset_cross_section(nTupleReader *r) { return r->resetCrossSection(); }
double get_cross_section(nTupleReader *r) { return r->getCrossSection(); }
double get_cross_section_error(nTupleReader *r) {
  return r->getCrossSectionError();
}
}
