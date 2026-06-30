---
description: Knowledge base for the Yang–Mills mass gap Clay Millennium problem. Includes proven results, active programs, failure modes of claimed solutions, and plain-language references across physics, lattice, and constructive QFT literature.
---

# Yang–Mills Mass Gap

A working knowledge base for the Clay Millennium Prize problem. See `yangmills.txt` at project root for the full living document.

## The Problem

Construct a non-trivial 4D quantum Yang–Mills theory (any compact simple gauge group G) satisfying Wightman or Osterwalder–Schrader axioms, and prove a mass gap Δ > 0 (strictly positive minimum energy above vacuum).

## Glossary

| Term | Meaning |
|---|---|
| Wightman axioms | 1964 formal definition of a legitimate QFT |
| OS axioms | Euclidean-time equivalent; includes reflection positivity |
| Mass gap (Δ) | Energy gap between vacuum and lowest excited state |
| Lattice gauge theory | Discretized spacetime, computer-simulable |
| Continuum limit | Lattice spacing → 0 (the unsolved part) |
| Asymptotic freedom | Coupling weakens at short distance (proven 1973) |
| Confinement | Quarks/gluons never isolated (physics-rigor, not constructive) |
| Wilson loop / string tension | Order parameter for confinement; area-law ⇒ confining |
| Transfer operator | Time evolution on lattice; spectral gap in it ≈ lattice Δ |
| Regularity structures | Hairer's machinery for ill-posed stochastic PDEs |
| Reflection positivity | Key OS condition; most rigor attempts hinge on it |

## Established Results

- **2D Yang–Mills**: fully rigorous construction (Lévy 2003; Chevyrev 2019)
- **3D Yang–Mills–Higgs**: rigorous stochastic construction with Higgs field (CCHS 2024)
- **3D state space for YM**: rigorous candidate defined (Cao–Chatterjee 2024)
- **Large-N lattice YM (strong coupling)**: rigorously solved (Chatterjee 2019)
- **Probabilistic confinement**: rigorous mechanism in restricted setting (Chatterjee 2021)
- **Mass gap on lattice**: numerically confirmed, not a continuum proof
- **Lightest glueball** (0⁺⁺ pure SU(3)): m ≈ 1.73 ± 0.07 GeV (Morningstar & Peardon 1999)

## Active Programs

| Program | Lead(s) | Frontier |
|---|---|---|
| Regularity structures / stochastic quantization | Hairer, Chevyrev, Chandra, Shen | Push to pure 3D, then 4D |
| Probabilistic/lattice methods | Chatterjee, Cao | Continuum limit via random surfaces |
| Geometric analysis (YM heat flow) | L. Gross | Extending heat-flow to 4D |
| Classical constructive QFT | Balaban; Federbush; Magnen–Rivasseau–Sénéor | Largely stalled |

## Failure Modes of Claimed Solutions

- **Pattern A**: redefine terms outside Wightman/OS, declare victory
- **Pattern B**: physics-style argument in math notation, skips 4D renormalization
- Recent claims (2024–26): none accepted by constructive-QFT community; most fail Pattern A or B

## Tools Available

- `verify_proof` — stub for calling external verifiers (Lean/Coq/Z3) on proof steps
- `search_literature` — search arXiv and web for papers by keyword/topic
- `explore_theorem` — decompose a theorem into sub-goals and track exploration state

## Usage

Load this skill when the user's task involves Yang–Mills theory, quantum field theory rigor, or the Clay Millennium problems. The living knowledge base `yangmills.txt` at project root contains the full citation-dense document with timeline, claimed-solution catalog, and open-question log.
