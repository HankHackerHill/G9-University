import type { ReactNode } from 'react';

export interface TableProps {
  caption: string;
  headers: readonly string[];
  rows: readonly { key: string; cells: readonly ReactNode[] }[];
  emptyText?: string;
  /** Zero-based indexes of the columns that hold measured numbers: right-aligned, machine voice. */
  numeric?: readonly number[];
  /** A totals row, drawn under a double rule. */
  footer?: readonly ReactNode[] | undefined;
}

/** Dense data table with a caption; the tabular alternative required for every chart. */
export function Table({ caption, headers, rows, emptyText, numeric = [], footer }: TableProps) {
  const cls = (i: number) => (numeric.includes(i) ? 'num' : undefined);
  return (
    <table>
      <caption>{caption}</caption>
      <thead>
        <tr>
          {headers.map((h, i) => (
            <th scope="col" key={h} className={cls(i)}>
              {h}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.length === 0 ? (
          <tr>
            <td colSpan={headers.length} className="muted">
              {emptyText ?? 'No rows.'}
            </td>
          </tr>
        ) : (
          rows.map((r) => (
            <tr key={r.key}>
              {r.cells.map((c, i) => (
                <td key={`${r.key}-${headers[i] ?? i}`} className={cls(i)}>
                  {c}
                </td>
              ))}
            </tr>
          ))
        )}
      </tbody>
      {footer === undefined ? null : (
        <tfoot>
          <tr>
            {footer.map((c, i) => (
              <td key={headers[i] ?? i} className={cls(i)}>
                {c}
              </td>
            ))}
          </tr>
        </tfoot>
      )}
    </table>
  );
}
