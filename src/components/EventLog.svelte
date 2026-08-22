<script lang="ts">
  import { PLAYER_NAMES } from '../data/types'
  import type { LogEntry } from '../data/log'

  interface Props {
    entries: LogEntry[]
  }

  let { entries }: Props = $props()
  let list: HTMLOListElement | null = $state(null)

  $effect(() => {
    void entries.length
    if (list) list.scrollTop = list.scrollHeight
  })
</script>

<div class="panel log">
  <span class="small-caps">Dispatches</span>
  <ol bind:this={list}>
    {#each entries as entry, index (index)}
      <li class:grave={entry.grave}>
        {#if entry.player >= 0}
          <span class="who" class:umber={entry.player === 1}>{PLAYER_NAMES[entry.player]}</span>
        {/if}
        {entry.text}
      </li>
    {:else}
      <li class="empty">No moves yet.</li>
    {/each}
  </ol>
</div>

<style>
  .log {
    padding: 0.7rem 0.85rem;
    display: grid;
    gap: 0.4rem;
    min-height: 0;
  }

  ol {
    list-style: none;
    overflow-y: auto;
    max-height: 15rem;
    display: grid;
    gap: 0.28rem;
    align-content: start;
    font-size: 0.9rem;
    line-height: 1.35;
  }

  li {
    color: var(--ink-soft);
  }

  li.grave {
    color: var(--crimson);
  }

  li.empty {
    font-style: italic;
    color: var(--ink-faint);
  }

  .who {
    font-family: var(--font-display);
    font-size: 0.75rem;
    letter-spacing: 0.06em;
    color: var(--ink);
    margin-right: 0.25rem;
  }

  .who.umber {
    color: var(--umber);
  }
</style>
