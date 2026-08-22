<script lang="ts">
  import PieceIcon from './PieceIcon.svelte'
  import TokenIcon from './TokenIcon.svelte'
  import { lineName } from '../data/log'
  import {
    CASTLE_COLS,
    PLAYER_NAMES,
    TOKEN_KINDS,
    siegeEngines,
    tokenOf,
  } from '../data/types'
  import type { GameState, PieceKind } from '../data/types'

  interface Props {
    game: GameState
    player: number
    seat: string
    active: boolean
    thinking?: boolean
  }

  let { game, player, seat, active, thinking = false }: Props = $props()

  let engines = $derived(siegeEngines(game, player))
  const SIEGE: PieceKind[] = ['trebuchet', 'batteringRam']
</script>

<div class="panel player" class:active class:umber={player === 1}>
  <header>
    <h3>{PLAYER_NAMES[player]}</h3>
    <span class="seat">{seat}{thinking ? ' · thinking…' : ''}</span>
  </header>

  <div class="row">
    <span class="small-caps">Castles</span>
    <span class="pips">
      {#each CASTLE_COLS as _, index (index)}
        <span class="pip" class:gone={!game.castles[player][index]}></span>
      {/each}
    </span>
  </div>

  <div class="row">
    <span class="small-caps">Siege</span>
    <span class="engines">
      {#each SIEGE as kind (kind)}
        <span class="engine" class:gone={!engines.includes(kind)} title={kind}>
          <PieceIcon {kind} size={20} />
        </span>
      {/each}
    </span>
  </div>

  <div class="tokens">
    {#each TOKEN_KINDS as kind (kind)}
      {@const token = tokenOf(game, player, kind)}
      <span class="token-chip" class:armed={token.face === 'attack'}>
        <TokenIcon {kind} face={token.face} size={18} />
        <span class="where">{lineName(kind, token.line)}</span>
      </span>
    {/each}
  </div>
</div>

<style>
  .player {
    padding: 0.7rem 0.85rem;
    display: grid;
    gap: 0.45rem;
    border-left: 3px solid var(--ivory-edge);
  }

  .player.umber {
    border-left-color: var(--umber);
  }

  .player.active {
    box-shadow: 0 0 0 2px var(--gold-soft), var(--shadow-soft);
  }

  header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 0.5rem;
  }

  h3 {
    font-size: 1.05rem;
  }

  .seat {
    font-size: 0.8rem;
    color: var(--ink-faint);
    font-style: italic;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.6rem;
  }

  .pips {
    display: flex;
    gap: 5px;
  }

  .pip {
    width: 13px;
    height: 13px;
    border-radius: 2px;
    background: var(--ink-soft);
  }

  .pip.gone {
    background: none;
    border: 1px dashed var(--ink-faint);
    opacity: 0.5;
  }

  .engines {
    display: flex;
    gap: 6px;
    color: var(--ink-soft);
  }

  .engine.gone {
    opacity: 0.22;
  }

  .tokens {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .token-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.15rem 0.4rem;
    border-radius: 999px;
    border: 1px solid var(--gold-soft);
    font-size: 0.78rem;
    color: var(--azure);
  }

  .token-chip.armed {
    color: var(--crimson);
    border-color: rgba(163, 34, 34, 0.4);
    background: rgba(163, 34, 34, 0.07);
  }

  .where {
    color: var(--ink-soft);
  }
</style>
