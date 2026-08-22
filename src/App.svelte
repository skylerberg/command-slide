<script lang="ts">
  import GameScreen from './components/GameScreen.svelte'
  import type { GameSetup } from './components/GameScreen.svelte'
  import MainMenu from './components/MainMenu.svelte'
  import { initEngine } from './engine/wasmEngine'

  let ready = $state(false)
  let failure: string | null = $state(null)
  let setup: GameSetup | null = $state(null)

  initEngine().then(
    () => (ready = true),
    (error) => (failure = String(error)),
  )
</script>

{#if failure}
  <div class="notice">
    <h1>The engine failed to load</h1>
    <p>{failure}</p>
    <p class="hint">Run <code>npm run build:wasm</code> and reload.</p>
  </div>
{:else if !ready}
  <div class="notice">
    <p>Mustering…</p>
  </div>
{:else if setup}
  {#key setup}
    <GameScreen {setup} onexit={() => (setup = null)} />
  {/key}
{:else}
  <MainMenu onstart={(chosen) => (setup = chosen)} />
{/if}

<style>
  .notice {
    min-height: 100dvh;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 0.5rem;
    text-align: center;
    padding: 2rem;
    color: var(--ink-soft);
    font-style: italic;
  }

  .hint {
    font-size: 0.9rem;
  }

  code {
    font-family: ui-monospace, monospace;
    font-style: normal;
    background: var(--parchment-deep);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
  }
</style>
