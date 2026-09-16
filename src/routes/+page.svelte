<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Popover from "$lib/Popover.svelte";
  import SettingsView from "$lib/SettingsView.svelte";

  let label = $state<string | null>(null);

  onMount(() => {
    label = getCurrentWindow().label;
  });
</script>

{#if label === "settings"}
  <SettingsView />
{:else if label === "main"}
  <Popover />
{/if}

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent;
  }
</style>
