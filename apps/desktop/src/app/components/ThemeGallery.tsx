import { useId, type CSSProperties } from 'react';
import { lowestTextContrast } from '../theme/contrast';
import { fixedScheme, paletteOf, previewStyle, SKIN_GROUPS, SKINS, type SkinId } from '../theme/skins';

/**
 * The themes as a radio group of previews. Each preview is drawn in its own theme (a
 * `.skin-scope` element with that theme's custom properties), so what you pick is what you get.
 * The contrast figure is computed from the palette (contrast.ts), the same check the theme
 * test asserts; it does not include textures such as scan lines.
 */
export function ThemeGallery({ value, onChange, scheme }: { value: SkinId; onChange: (id: SkinId) => void; scheme: 'light' | 'dark' }) {
  const name = useId();
  return (
    <fieldset className="skin-gallery">
      <legend className="visually-hidden">Theme</legend>
      {SKIN_GROUPS.map((group) => (
        <div key={group.id} className="skin-group" role="group" aria-labelledby={`${name}-${group.id}`}>
          <h3 id={`${name}-${group.id}`}>{group.title}</h3>
          <div className="skin-grid">
            {SKINS.filter((s) => s.group === group.id).map((skin) => {
              const fixed = fixedScheme(skin);
              const shown = fixed ?? scheme;
              const lowest = lowestTextContrast(paletteOf(skin, shown));
              const id = `${name}-${skin.id}`;
              return (
                <label key={skin.id} className="skin-option" htmlFor={id}>
                  <span className="skin-option-head">
                    <input
                      type="radio"
                      id={id}
                      name={name}
                      value={skin.id}
                      checked={value === skin.id}
                      aria-labelledby={`${id}-name`}
                      aria-describedby={`${id}-desc ${id}-meta`}
                      onChange={() => {
                        onChange(skin.id);
                      }}
                    />
                    <span className="skin-name" id={`${id}-name`}>
                      {skin.name}
                    </span>
                  </span>
                  <span
                    className="skin-card skin-scope"
                    style={previewStyle(skin, shown)}
                    data-regions={skin.regions}
                    data-effect={skin.effect}
                    aria-hidden="true"
                  >
                    {skin.regions === 'window' ? <span className="skin-bar">Mission Control</span> : null}
                    <span className="skin-title">D15</span>
                    <span className="skin-line">Credited 1:15:00 of 2:00:00</span>
                    <span className="meter" style={{ '--posted': 0.55, '--doubted': 0.2 } as CSSProperties} />
                    <span className="skin-marks">
                      <span className="badge badge-verified">VERIFIED</span>
                      <span className="badge badge-proposed">PROPOSED</span>
                      <span className="badge badge-recommendation">ADVICE</span>
                      <span className="badge badge-state" data-state="FAILED">
                        FAILED
                      </span>
                    </span>
                  </span>
                  <span className="skin-desc" id={`${id}-desc`}>
                    {skin.description}
                  </span>
                  <span className="skin-meta" id={`${id}-meta`}>
                    {fixed === null ? 'Light or dark, as set.' : fixed === 'dark' ? 'Dark.' : 'Light.'} Text contrast {lowest.toFixed(1)}:1 or more.
                  </span>
                </label>
              );
            })}
          </div>
        </div>
      ))}
    </fieldset>
  );
}
