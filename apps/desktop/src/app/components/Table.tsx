import type { ReactNode } from 'react';

export interface TableProps {
  caption: string;
  headers: readonly string[];
  rows: readonly { key: string; cells: readonly ReactNode[] }[];
  emptyText?: string;
}

/** Dense data table with a caption; the tabular alternative required for every chart. */
export function Table({ caption, headers, rows, emptyText }: TableProps) {
  return (
    <table>
      <caption>{caption}</caption>
      <thead>
        <tr>
          {headers.map((h) => (
            <th scope="col" key={h}>
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
              {r.cells.map((c, i) => <td key={`${r.key}-${headers[i] ?? i}`}>{c}</td>)}
            </tr>
          ))
        )}
      </tbody>
    </table>
  );
}
