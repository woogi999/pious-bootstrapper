<script lang="ts">
  import { bytes } from "../lib/format";
  import type { Install } from "../lib/types";

  let { install }: { install: Install } = $props();
  const fraction = $derived(install.total ? install.downloaded / install.total : 0);
</script>

<div class="col" style="gap: 6px">
  <div class="row">
    <span class="meta grow line">{install.stage}</span>
    <span class="secondary">
      {install.total ? `${bytes(install.downloaded)} of ${bytes(install.total)} · ${Math.round(fraction * 100)}%` : "Preparing…"}
    </span>
  </div>
  <div class="progress"><div style="width: {fraction * 100}%"></div></div>
</div>
