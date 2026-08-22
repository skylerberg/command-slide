<script lang="ts">
  import PieceIcon from './PieceIcon.svelte'
  import RulesOverlay from './RulesOverlay.svelte'
  import { DIFFICULTIES } from '../ai/aiController'
  import { PLAYER_NAMES } from '../data/types'
  import type { GameSetup } from './GameScreen.svelte'

  interface Props {
    onstart: (setup: GameSetup) => void
  }

  let { onstart }: Props = $props()

  type Mode = 'solo' | 'hotseat' | 'watch'

  let mode: Mode = $state('solo')
  let side = $state(0)
  let level = $state(1)
  let showRules = $state(false)

  function start() {
    const difficulty = DIFFICULTIES[level]
    const seats: ('human' | 'ai')[] =
      mode === 'hotseat'
        ? ['human', 'human']
        : mode === 'watch'
          ? ['ai', 'ai']
          : side === 0
            ? ['human', 'ai']
            : ['ai', 'human']
    onstart({ seats, iterations: difficulty.iterations, difficulty: difficulty.name })
  }
</script>

<div class="menu">
  <div class="panel card">
    <div class="crest">
      <PieceIcon kind="trebuchet" size={44} />
    </div>
    <h1>Command Slide</h1>
    <p class="tagline">
      Two command tokens, one board, and a siege that turns on where you point them.
    </p>

    <fieldset>
      <legend class="small-caps">Opponent</legend>
      <div class="options">
        <label class:picked={mode === 'solo'}>
          <input type="radio" bind:group={mode} value="solo" />
          <span>Play the machine</span>
        </label>
        <label class:picked={mode === 'hotseat'}>
          <input type="radio" bind:group={mode} value="hotseat" />
          <span>Two players, one screen</span>
        </label>
        <label class:picked={mode === 'watch'}>
          <input type="radio" bind:group={mode} value="watch" />
          <span>Watch two machines</span>
        </label>
      </div>
    </fieldset>

    {#if mode === 'solo'}
      <fieldset>
        <legend class="small-caps">Your side</legend>
        <div class="options row">
          {#each PLAYER_NAMES as name, index (name)}
            <label class:picked={side === index}>
              <input type="radio" bind:group={side} value={index} />
              <span>{name}{index === 0 ? ' · moves first' : ''}</span>
            </label>
          {/each}
        </div>
      </fieldset>
    {/if}

    {#if mode !== 'hotseat'}
      <fieldset>
        <legend class="small-caps">Strength</legend>
        <div class="options">
          {#each DIFFICULTIES as difficulty, index (difficulty.name)}
            <label class:picked={level === index}>
              <input type="radio" bind:group={level} value={index} />
              <span>
                <strong>{difficulty.name}</strong>
                <em>{difficulty.blurb}</em>
              </span>
            </label>
          {/each}
        </div>
      </fieldset>
    {/if}

    <div class="actions">
      <button class="primary" onclick={start}>Begin the siege</button>
      <button onclick={() => (showRules = true)}>How to play</button>
    </div>
  </div>
</div>

{#if showRules}
  <RulesOverlay onclose={() => (showRules = false)} />
{/if}

<style>
  .menu {
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 1.5rem;
  }

  .card {
    width: min(30rem, 100%);
    padding: 1.6rem 1.7rem 1.8rem;
    display: grid;
    gap: 1rem;
    text-align: center;
  }

  .crest {
    display: grid;
    place-items: center;
    color: var(--ink-soft);
  }

  h1 {
    font-size: 1.9rem;
  }

  .tagline {
    color: var(--ink-soft);
    font-style: italic;
    margin-top: -0.5rem;
  }

  fieldset {
    border: none;
    display: grid;
    gap: 0.4rem;
    text-align: left;
  }

  legend {
    margin-bottom: 0.25rem;
  }

  .options {
    display: grid;
    gap: 0.35rem;
  }

  .options.row {
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
  }

  label {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.45rem 0.6rem;
    border: 1px solid var(--gold-soft);
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }

  label:hover {
    background: var(--parchment-deep);
  }

  label.picked {
    border-color: var(--gold);
    background: rgba(185, 143, 46, 0.1);
  }

  label span {
    display: grid;
  }

  label em {
    font-size: 0.85rem;
    color: var(--ink-faint);
  }

  input {
    accent-color: var(--gold);
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: center;
    margin-top: 0.4rem;
  }
</style>
