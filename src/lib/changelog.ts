// The changelog (CHANGELOG.md, built into the app), split into versions.

export interface Release {
  version: string;
  /** Headings and bullet lists, in order. */
  blocks: ({ kind: "heading"; text: string } | { kind: "list"; items: string[] } | { kind: "text"; text: string })[];
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
