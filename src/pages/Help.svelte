<!-- Help: every Pious document, inside Pious (they're part of the app, so
     they work offline and always match this version). Links between
     documents open here; web links open where links open (Settings). -->
<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/state.svelte";
  import { run } from "../lib/api";
  import { render } from "../lib/markdown";
  import Icon from "../components/Icon.svelte";

  let { doc = undefined }: { doc?: string } = $props();

  const files = {
    ...(import.meta.glob("../../docs/*.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>),
    ...(import.meta.glob(["../../PRIVACY.md", "../../TERMS.md"], { query: "?raw", import: "default", eager: true }) as Record<string, string>),
  };

  /** The documents, in reading order: file name → title, icon. */
  const ORDER: [string, string, string][] = [
    ["INSTALL", "Installing and updating", "download"],
    ["CONFIGURATION", "Settings and your files", "settings"],
    ["TWEAKS", "Tweaks", "lightning"],
    ["CUSTOMIZATION", "Customizing Pious", "sparkles"],
    ["THEMES", "Making a theme", "picture"],
    ["MODS", "Roblox mods and FastFlags", "folder"],
    ["PLUGINS", "Plugins and making one", "puzzle"],
    ["PLUGIN-API", "Plugin API reference", "command-line"],
    ["PLUGIN-UI", "Plugin UI kit", "sparkles"],
    ["FEATURES", "Notifications, regions and more", "bell"],
    ["MCP", "AI apps (MCP)", "sparkles"],
    ["TROUBLESHOOTING", "Troubleshooting", "warning"],
    ["PRIVACY", "Privacy Policy", "shield"],
    ["TERMS", "Terms of Service", "info"],
  ];
  const docs = Object.entries(files)
    .map(([path, text]) => {
      const name = path.split("/").pop()!.replace(/\.md$/, "");
      const known = ORDER.findIndex(([n]) => n === name);
      const title = known >= 0 ? ORDER[known][1] : (/^#\s+(.*)$/m.exec(text)?.[1] ?? name);
      return { name, title, icon: known >= 0 ? ORDER[known][2] : "info", text, order: known >= 0 ? known : 99 };
    })
    .sort((a, b) => a.order - b.order || a.title.localeCompare(b.title));

  let current = $state(docs.find((d) => d.name === doc?.split("#")[0])?.name ?? docs[0]?.name ?? "");
  let query = $state("");
  let body = $state<HTMLElement>();

  const page = $derived(docs.find((d) => d.name === current) ?? docs[0]);
  const rendered = $derived(page ? render(page.text) : { html: "", headings: [] });
  const matches = $derived(
    query.trim()
      ? docs.filter((d) => d.text.toLowerCase().includes(query.trim().toLowerCase()) || d.title.toLowerCase().includes(query.trim().toLowerCase()))
      : docs,
  );

  async function open(name: string, anchor?: string) {
    current = name;
    await tick();
    const target = anchor ? body?.querySelector(`#${CSS.escape(anchor)}`) : null;
    if (target) target.scrollIntoView({ behavior: "smooth", block: "start" });
    else document.getElementById("page-scroll")?.scrollTo({ top: 0 });
    // Highlight what was searched for, so it's easy to spot.
    if (query.trim() && body) {
      const found = [...body.querySelectorAll("p, li, td, h2, h3")].find((el) => el.textContent?.toLowerCase().includes(query.trim().toLowerCase()));
      found?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }

  // An anchor asked for on the way in (e.g. "PLUGINS#permissions").
  $effect(() => {
    const anchor = doc?.split("#")[1];
    if (anchor) tick().then(() => body?.querySelector(`#${CSS.escape(anchor)}`)?.scrollIntoView({ block: "start" }));
  });

  function click(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a");
    if (!link) return;
    event.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (/^https?:\/\//.test(href)) {
      run("open_url", { url: href });
      return;
    }
    if (href.startsWith("#")) {
      body?.querySelector(`#${CSS.escape(href.slice(1))}`)?.scrollIntoView({ behavior: "smooth" });
      return;
    }
    // Another document: "docs/PLUGINS.md", "../PRIVACY.md", "THEMES.md#colors".
    const [file, anchor] = href.split("#");
    const name = file.split("/").pop()?.replace(/\.md$/i, "") ?? "";
    if (docs.some((d) => d.name === name)) open(name, anchor);
    else if (/README|CHANGELOG/i.test(name)) ((app.settingsTab = "about"), app.navigate({ name: "settings" }));
  }
</script>

<div class="page help">
  <div class="page-header">
    <div class="col grow" style="gap: 2px">
      <h1 class="page-title">Help</h1>
      <span class="meta">Everything about Pious: setting it up, tweaks, themes, plugins and fixing problems</span>
    </div>
  </div>

  <div class="layout">
    <nav class="glass toc">
      <label class="search-box glass-base">
        <Icon name="search" />
        <input placeholder="Search the help" bind:value={query} />
      </label>
      {#each matches as d (d.name)}
        <button class="doc" class:on={d.name === current} onclick={() => open(d.name)}><Icon name={d.icon} />{d.title}</button>
      {:else}
        <span class="meta empty">Nothing in the help mentions that.</span>
      {/each}
      {#if rendered.headings.filter((h) => h.level === 2).length > 1}
        <hr class="divider" />
        <span class="label">On this page</span>
        {#each rendered.headings.filter((h) => h.level === 2) as h (h.id)}
          <button class="anchor" onclick={() => open(current, h.id)}>{h.text}</button>
        {/each}
      {/if}
    </nav>

    <!-- Clicks on the document's links (real <a>s, so keyboards reach them). -->
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
    <article class="glass doc-body selectable" bind:this={body} onclick={click}>
      {@html rendered.html}
    </article>
  </div>
</div>

<style>
  .help {
    max-width: 1180px;
    margin: 0 auto;
    width: 100%;
    gap: 16px;
  }
  .layout {
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .toc {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px;
    max-height: calc(100vh - 140px);
    overflow-y: auto;
  }
  .search-box {
    margin-bottom: 6px;
  }
  .doc,
  .anchor {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    border-radius: var(--r-sm);
    background: none;
    color: rgb(var(--muted));
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .doc:hover,
  .anchor:hover {
    background: rgb(var(--surface) / 0.06);
    color: rgb(var(--text));
  }
  .doc.on {
    background: rgb(var(--surface) / 0.1);
    color: rgb(var(--text));
    font-weight: 600;
  }
  .anchor {
    font-size: 12px;
    padding: 4px 8px 4px 12px;
  }
  .label {
    padding: 4px 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: rgb(var(--faint));
  }
  .empty {
    padding: 8px;
  }
  .doc-body {
    padding: 22px 28px 30px;
    line-height: 1.6;
    font-size: 13.5px;
    min-width: 0;
  }
  .doc-body :global(h1) {
    font-size: 24px;
    margin: 0 0 12px;
  }
  .doc-body :global(h2) {
    font-size: 18px;
    margin: 26px 0 8px;
    padding-top: 6px;
    border-top: 1px solid rgb(var(--surface) / 0.06);
  }
  .doc-body :global(h3) {
    font-size: 15px;
    margin: 18px 0 6px;
  }
  .doc-body :global(p),
  .doc-body :global(li) {
    color: rgb(var(--text) / 0.88);
  }
  .doc-body :global(a) {
    color: rgb(var(--accent));
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .doc-body :global(code) {
    font-family: "Cascadia Code", Consolas, monospace;
    font-size: 12px;
    padding: 1px 5px;
    border-radius: 5px;
    background: rgb(var(--surface) / 0.08);
  }
  .doc-body :global(pre) {
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: rgb(0 0 0 / 0.28);
    overflow-x: auto;
  }
  .doc-body :global(pre code) {
    padding: 0;
    background: none;
  }
  .doc-body :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 10px 0;
    font-size: 12.5px;
  }
  .doc-body :global(th),
  .doc-body :global(td) {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid rgb(var(--surface) / 0.07);
    vertical-align: top;
  }
  .doc-body :global(th) {
    color: rgb(var(--faint));
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .doc-body :global(blockquote) {
    margin: 10px 0;
    padding: 4px 14px;
    border-left: 3px solid rgb(var(--accent) / 0.6);
    background: rgb(var(--surface) / 0.03);
    border-radius: 0 var(--r-sm) var(--r-sm) 0;
  }
  @media (max-width: 860px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .toc {
      position: static;
      max-height: none;
    }
  }
</style>
