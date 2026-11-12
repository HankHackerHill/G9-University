/**
 * Theme preference (`g9.skin`). The theme is applied whenever it or the contrast setting
 * changes; `applySkin` is idempotent, so every screen that shows the setting may call this.
 */
import { useEffect } from 'react';
import { parsePreference, usePreference } from './preferences';
import { applySkin, DEFAULT_SKIN, SKIN_IDS, SKIN_STORAGE_KEY, skinById, type SkinId } from './skins';
import { CONTRAST_MODES, CONTRAST_STORAGE_KEY, type ContrastMode } from './useContrast';

export function readStoredSkin(storage: Pick<Storage, 'getItem'> | null): SkinId {
  try {
    return parsePreference(storage?.getItem(SKIN_STORAGE_KEY) ?? null, SKIN_IDS, DEFAULT_SKIN);
  } catch {
    return DEFAULT_SKIN;
  }
}

export function useSkin(): [SkinId, (id: SkinId) => void] {
  const [skin, setSkin] = usePreference<SkinId>(SKIN_STORAGE_KEY, SKIN_IDS, DEFAULT_SKIN);
  const [contrast] = usePreference<ContrastMode>(CONTRAST_STORAGE_KEY, CONTRAST_MODES, 'default');

  useEffect(() => {
    applySkin(skinById(skin), contrast);
  }, [skin, contrast]);

  return [skin, setSkin];
}
