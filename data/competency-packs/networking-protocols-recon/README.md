# Knowledge pack: Networking, protocol analysis and scoped reconnaissance

**Status:** PROPOSED (v0.1.0) · **Owner module:** M12 · **Seed curriculum weeks:** Week 2 · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
| `net.layers-addressing` | Layers, addressing and subnetting | 1–2 | — | 240 |
| `net.tcp-states` | TCP states and failure interpretation | 2–3 | net.layers-addressing | 180 |
| `net.dns-resolution` | DNS resolution and caching | 2–3 | net.layers-addressing | 180 |
| `net.routing-segmentation` | Routing, NAT and segmentation | 2–4 | net.tcp-states | 240 |
| `recon.scope-definition` | Reconnaissance scope definition | 2–3 | linux-python-systems:lab.authorization | 180 |
| `recon.discovery-interpretation` | Discovery result interpretation | 3–4 | recon.scope-definition, net.routing-segmentation | 180 |
| `net.pcap-analysis` | Unfamiliar PCAP analysis | 3–5 | net.tcp-states, net.dns-resolution | 180 |
| `net.failure-diagnosis` | Network failure diagnosis | 3–5 | net.pcap-analysis, recon.discovery-interpretation | 180 |

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture (VM), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

Flow records, packet captures, firewall and DNS logs; scoped discovery produces an auditable target list and timing.

## Safety boundaries

- Reconnaissance only against explicitly scoped lab targets
- No scanning from the G9 host; use the isolated lab network
- Capture files may contain credentials: classify PRIVATE

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
| RFC 793 / RFC 9293 TCP | IETF | STANDARD | public RFC | ASSUMED |
| Wireshark User's Guide | Wireshark Foundation | DOC | GPL documentation | ASSUMED |
| Nmap Reference Guide | Nmap Project | DOC | freely available; tool license is NPSL | ASSUMED |

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
