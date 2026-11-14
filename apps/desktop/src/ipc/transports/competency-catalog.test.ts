// @vitest-environment node
/**
 * The mock's competency catalog is a copy of the packs that ship with the application (the
 * pack.json file of each directory under data/competency-packs, embedded by g9-app). It must say
 * what the packs say.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import catalog from '../../fixtures/competency-catalog.json';

const PACK_DIR = fileURLToPath(new URL('../../../../../data/competency-packs/', import.meta.url));

interface PackFile {
  pack_id: string;
  title: string;
  version: string;
  status: string;
  competencies: { id: string; name: string; prerequisites: string[] }[];
}

function packsOnDisk(): PackFile[] {
  return readdirSync(PACK_DIR)
    .sort()
    .map((dir) => JSON.parse(readFileSync(`${PACK_DIR}${dir}/pack.json`, 'utf8')) as PackFile);
}

describe('competency catalog of the mock transport', () => {
  it('holds exactly the packs, titles, versions, competencies and prerequisites of the files on disk', () => {
    const disk = packsOnDisk().map((p) => ({
      pack_id: p.pack_id,
      title: p.title,
      version: p.version,
      status: p.status,
      competencies: p.competencies.map((c) => ({ id: c.id, name: c.name, prerequisites: c.prerequisites })),
    }));
    expect(catalog.packs).toEqual(disk);
  });

  it('is eight packs of eight competencies with unique ids', () => {
    expect(catalog.packs).toHaveLength(8);
    const ids = catalog.packs.flatMap((p) => p.competencies.map((c) => c.id));
    expect(ids).toHaveLength(64);
    expect(new Set(ids).size).toBe(64);
  });
});
