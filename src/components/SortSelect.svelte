<!-- How a page's list is sorted; remembered per page. -->
<script lang="ts">
  import { setPreferences } from "../lib/api";
  import Select from "./Select.svelte";

  let {
    page,
    value,
    options,
    width = "180px",
    onchange,
  }: {
    page: string;
    value: string;
    options: { value: string; label: string }[];
    width?: string;
    /** For pages that keep their sort themselves. */
    onchange?: (value: string) => void;
  } = $props();
</script>

<Select
  options={options.map((o) => ({ value: o.value, label: `Sort: ${o.label}` }))}
  {value}
  {width}
  onchange={(v) => (onchange ? onchange(v) : setPreferences({ sorts: { [page]: v } }))}
/>
