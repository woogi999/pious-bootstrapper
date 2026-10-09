// The changelog (CHANGELOG.md, built into the app), split into versions.

export interface Release {
  version: string;
  /** Headings and bullet lists, in order. */
  blocks: ({ kind: "heading"; text: string } | { kind: "list"; items: string[] } | { kind: "text"; text: string })[];
}

/** A piece of a line: plain, **bold**, `code`, or a [link](…) (shown as its text). */
export type Span = { kind: "text" | "bold" | "code"; text: string };

/** Splits a line's Markdown into pieces to draw (never as HTML). */
export function spans(line: string): Span[] {
  const out: Span[] = [];
  const pattern = /\*\*(.+?)\*\*|`([^`]+)`|\[([^\]]+)\]\([^)]*\)/g;
  let at = 0;
  for (const match of line.matchAll(pattern)) {
    if (match.index! > at) out.push({ kind: "text", text: line.slice(at, match.index) });
    if (match[1] !== undefined) out.push({ kind: "bold", text: match[1] });
    else if (match[2] !== undefined) out.push({ kind: "code", text: match[2] });
    else out.push({ kind: "text", text: match[3] });
    at = match.index! + match[0].length;
  }
  if (at < line.length) out.push({ kind: "text", text: line.slice(at) });
  return out;
}

export function parseChangelog(text: string): Release[] {
  const releases: Release[] = [];
  let current: Release | null = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const version = line.match(/^##\s+(.+)$/);
    if (version) {
      current = { version: version[1].trim(), blocks: [] };
      releases.push(current);
      continue;
    }
    if (!current || !line.trim()) continue;
    const heading = line.match(/^###\s+(.+)$/);
    const item = line.match(/^\s*[-*]\s+(.+)$/);
    const last = current.blocks[current.blocks.length - 1];
    if (heading) current.blocks.push({ kind: "heading", text: heading[1] });
    else if (item) {
      if (last?.kind === "list") last.items.push(item[1]);
      else current.blocks.push({ kind: "list", items: [item[1]] });
    } else current.blocks.push({ kind: "text", text: line.trim() });
  }
  return releases;
}
