// A small Markdown renderer for Pious's own documentation (Help page): the
// parts the docs use — headings, paragraphs, lists, tables, code, quotes,
// links, bold and italics. Text is escaped first, so a document can't put
// HTML in the page. Links stay plain `<a>`s; the Help page decides where they
// go (another document, or the browser).

const escape = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** A heading's ID, for links within a page ("#making-a-theme"). */
export function slug(text: string): string {
  return text
    .toLowerCase()
    .replace(/<[^>]+>/g, "")
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-");
}

function inline(text: string): string {
  const codes: string[] = [];
  let out = escape(text).replace(/`([^`]+)`/g, (_, code) => {
    codes.push(code);
    return `\u0000${codes.length - 1}\u0000`;
  });
  out = out
    .replace(/!\[([^\]]*)\]\(([^)\s]+)[^)]*\)/g, (_, alt) => alt) // pictures: their text only
    .replace(/\[([^\]]+)\]\(([^)\s]+)[^)]*\)/g, (_, label, href) => `<a href="${href}">${label}</a>`)
    .replace(/&lt;(https?:\/\/[^&\s]+)&gt;/g, (_, href) => `<a href="${href}">${href}</a>`)
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/(^|[^*\w])\*([^*\s][^*]*)\*/g, "$1<em>$2</em>")
    .replace(/(^|\W)_([^_\s][^_]*)_(?=\W|$)/g, "$1<em>$2</em>");
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[Number(i)]}</code>`);
}

/** Markdown → HTML, and the document's headings (for its contents). */
export function render(markdown: string): { html: string; headings: { level: number; text: string; id: string }[] } {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const html: string[] = [];
  const headings: { level: number; text: string; id: string }[] = [];
  let i = 0;
  const isBlockStart = (l: string) => /^(#{1,6}\s|\s*```|>|\s*([-*+]|\d+\.)\s|\|.*\||---+\s*$|<p\s)/.test(l);

  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    // Fenced code.
    if (/^\s*```/.test(line)) {
      const code: string[] = [];
      i++;
      const indent = /^\s*/.exec(line)![0].length;
      // A fence indented inside a list item: its lines lose that indent.
      const outdent = (l: string) => l.slice(Math.min(indent, /^\s*/.exec(l)![0].length));
      while (i < lines.length && !/^\s*```/.test(lines[i])) code.push(outdent(lines[i++]));
      i++;
      html.push(`<pre><code>${escape(code.join("\n"))}</code></pre>`);
      continue;
    }
    // Headings.
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      const level = heading[1].length;
      const text = heading[2].trim();
      const id = slug(text);
      headings.push({ level, text: text.replace(/`/g, ""), id });
      html.push(`<h${level} id="${id}">${inline(text)}</h${level}>`);
      i++;
      continue;
    }
    // A raw HTML line (the README's logo): left out.
    if (/^<\/?[a-z]/i.test(line.trim())) {
      i++;
      continue;
    }
    if (/^---+\s*$/.test(line)) {
      html.push("<hr />");
      i++;
      continue;
    }
    // Quotes.
    if (line.startsWith(">")) {
      const quote: string[] = [];
      while (i < lines.length && lines[i].startsWith(">")) quote.push(lines[i++].replace(/^>\s?/, ""));
      html.push(`<blockquote>${render(quote.join("\n")).html}</blockquote>`);
      continue;
    }
    // Tables.
    if (/^\s*\|.*\|\s*$/.test(line) && i + 1 < lines.length && /^\s*\|?[\s:-]+\|/.test(lines[i + 1])) {
      const cells = (l: string) => l.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
      const head = cells(line);
      i += 2;
      const rows: string[][] = [];
      while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) rows.push(cells(lines[i++]));
      html.push(
        `<table><thead><tr>${head.map((h) => `<th>${inline(h)}</th>`).join("")}</tr></thead><tbody>${rows
          .map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`)
          .join("")}</tbody></table>`,
      );
      continue;
    }
    // Lists (nested by indentation).
    if (/^\s*([-*+]|\d+\.)\s/.test(line)) {
      const items: { indent: number; ordered: boolean; text: string }[] = [];
      while (i < lines.length && (/^\s*([-*+]|\d+\.)\s/.test(lines[i]) || (/^\s{2,}[^\s`]/.test(lines[i]) && items.length))) {
        const m = /^(\s*)([-*+]|\d+\.)\s+(.*)$/.exec(lines[i]);
        if (m) items.push({ indent: m[1].length, ordered: /\d/.test(m[2]), text: m[3] });
        else items[items.length - 1].text += " " + lines[i].trim();
        i++;
      }
      const build = (from: number, indent: number): [string, number] => {
        const ordered = items[from].ordered;
        let out = ordered ? "<ol>" : "<ul>";
        let k = from;
        while (k < items.length && items[k].indent >= indent) {
          if (items[k].indent > indent) {
            const [nested, next] = build(k, items[k].indent);
            out = out.replace(/<\/li>$/, nested + "</li>");
            k = next;
            continue;
          }
          out += `<li>${inline(items[k].text)}</li>`;
          k++;
        }
        return [out + (ordered ? "</ol>" : "</ul>"), k];
      };
      let k = 0;
      while (k < items.length) {
        const [list, next] = build(k, items[k].indent);
        html.push(list);
        k = next;
      }
      continue;
    }
    // A paragraph: lines up to a blank line or another block.
    const paragraph: string[] = [line.trim()];
    i++;
    while (i < lines.length && lines[i].trim() && !isBlockStart(lines[i])) paragraph.push(lines[i++].trim());
    html.push(`<p>${inline(paragraph.join(" "))}</p>`);
  }
  return { html: html.join("\n"), headings };
}
