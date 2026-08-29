<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import Board from './Board.svelte'
  import EventLog from './EventLog.svelte'
  import PlayerPanel from './PlayerPanel.svelte'
  import RulesOverlay from './RulesOverlay.svelte'
  import { AiController } from '../ai/aiController'
  import {
    applyChoice,
    attackPreview,
    forcedOrder,
    initialState,
    legalChoices,
    moveOutcomes,
    pendingAttackers,
    slideOutcomes,
  } from '../engine/wasmEngine'
  import { describeEvent, destroyedSquares, lineName, squareName } from '../data/log'
  import type { LogEntry } from '../data/log'
  import {
    PIECE_NAMES,
    PLAYER_NAMES,
    armedToken,
    pendingTokens,
    pieceAt,
    tokenOf,
    volleyPending,
  } from '../data/types'
  import type {
    AttackPreview,
    Choice,
    GameState,
    MoveOutcome,
    SlideOutcome,
    Square,
    TokenKind,
  } from '../data/types'

  export interface GameSetup {
    seats: ('human' | 'ai')[]
    iterations: number
    difficulty: string
  }

  interface Props {
    setup: GameSetup
    onexit: () => void
  }

  let { setup, onexit }: Props = $props()

  /** Pause between the AI's three decisions so a turn can be followed. */
  const STEP_MS = 480
  const FLASH_MS = 750

  interface Snapshot {
    game: GameState
    entries: LogEntry[]
  }

  let game: GameState = $state(initialState())
  let entries: LogEntry[] = $state([])
  let destroyed: Square[] = $state([])
  let history: Snapshot[] = $state([])
  /** True from the moment a search starts until the AI's turn is on the board. */
  let thinking = $state(false)
  let failure: string | null = $state(null)
  let showRules = $state(false)
  let showThreats = $state(true)
  let showOutcomes = $state(true)
  /** A sentence about whatever is under the cursor on the board. */
  let hint: string | null = $state(null)

  const ai = new AiController()
  /** Bumped whenever the position the AI is searching stops being current. */
  let generation = 0
  let flashTimer: ReturnType<typeof setTimeout> | null = null

  let humanSeats = $derived(
    setup.seats.map((seat, index) => (seat === 'human' ? index : -1)).filter((i) => i >= 0),
  )
  let isHumanTurn = $derived(
    game.outcome === null && setup.seats[game.currentPlayer] === 'human' && !thinking,
  )
  let choices: Choice[] = $derived(isHumanTurn ? legalChoices(game) : [])
  let mustPass = $derived(choices.length === 1 && choices[0].type === 'pass')

  /** The piece now choosing a target, while a volley walks its line. */
  let attackerSquare = $derived.by(() => {
    for (const choice of choices) {
      if (choice.type === 'attack' || choice.type === 'holdFire') return choice.from
    }
    return null
  })

  // When one seat is human the board keeps that side's perspective even while
  // the opponent thinks; a hot-seat game follows whoever is to move.
  let viewer = $derived(humanSeats.length === 1 ? humanSeats[0] : game.currentPlayer)

  let volley = $derived.by(() => {
    const result: Record<number, AttackPreview | null> = { 0: null, 1: null }
    for (const player of [0, 1]) {
      const kind = armedToken(game, player)
      result[player] = kind ? attackPreview(game, player, kind) : null
    }
    return result
  })

  let orderChoices = $derived(choices.filter((choice) => choice.type === 'order'))

  let outcomes: MoveOutcome[] = $derived(
    isHumanTurn && game.phase === 'activate' ? moveOutcomes($state.snapshot(game)) : [],
  )
  let slides: SlideOutcome[] = $derived(
    isHumanTurn && game.phase === 'slide' ? slideOutcomes($state.snapshot(game)) : [],
  )
  let attackers: Square[] = $derived(
    game.phase === 'attack' ? pendingAttackers($state.snapshot(game)) : [],
  )

  function choose(type: 'pass' | 'holdFire') {
    const choice = choices.find((option) => option.type === type)
    if (choice) handleChoice(choice)
  }

  function apply(choice: Choice) {
    hint = null
    const result = applyChoice($state.snapshot(game), choice)
    game = result.state
    for (const event of result.events) {
      const entry = describeEvent(event)
      if (entry) entries = [...entries, entry]
    }
    const struck = destroyedSquares(result.events)
    if (struck.length > 0) {
      destroyed = struck
      if (flashTimer) clearTimeout(flashTimer)
      flashTimer = setTimeout(() => (destroyed = []), FLASH_MS)
    }
  }

  function handleChoice(choice: Choice) {
    if (!isHumanTurn) return
    history = [...history, { game: $state.snapshot(game), entries: [...entries] }]
    apply(choice)
    advance()
  }

  /** Play out every decision that has only one possible outcome, so the player
      is only ever asked something they could answer more than one way. Nothing
      taken here reaches the undo stack: undo steps back to a real decision. */
  function autoResolve() {
    // A turn is a handful of decisions and a hot-seat game hands straight over.
    // The bound is only here so that a bug cannot freeze the tab.
    for (let step = 0; step < 64; step++) {
      if (game.outcome || thinking) return
      if (setup.seats[game.currentPlayer] !== 'human') return

      const position = $state.snapshot(game)
      const options = legalChoices(position)
      if (options.length === 1) {
        apply(options[0])
        continue
      }
      if (position.phase !== 'order') return
      // The engine settles the order whenever one of them gives up nothing;
      // only a fork the player could answer either way reaches them.
      const first = forcedOrder(position)
      if (!first) return
      apply({ type: 'order', first })
    }
  }

  function advance() {
    autoResolve()
    driveAi()
  }

  function driveAi() {
    if (game.outcome || thinking) return
    if (setup.seats[game.currentPlayer] !== 'ai') return
    void runAi()
  }

  async function runAi() {
    thinking = true
    const gen = generation
    try {
      const turn = await ai.takeTurn($state.snapshot(game), setup.iterations)
      if (gen !== generation) return
      for (const [index, choice] of turn.choices.entries()) {
        if (gen !== generation) return
        apply(choice)
        if (index < turn.choices.length - 1) {
          await new Promise((resolve) => setTimeout(resolve, STEP_MS))
        }
      }
    } catch (error) {
      if (gen === generation) failure = String(error)
    } finally {
      if (gen === generation) thinking = false
    }
    // Two machines playing each other hand the turn straight back.
    if (gen === generation) advance()
  }

  function undo() {
    const previous = history[history.length - 1]
    if (!previous) return
    generation += 1
    ai.cancel()
    thinking = false
    destroyed = []
    hint = null
    game = previous.game
    entries = previous.entries
    history = history.slice(0, -1)
  }

  function restart() {
    generation += 1
    ai.cancel()
    thinking = false
    destroyed = []
    hint = null
    entries = []
    history = []
    failure = null
    game = initialState()
    advance()
  }

  function tokenWord(kind: TokenKind): string {
    return kind === 'row' ? 'rank' : 'file'
  }

  function orderLabel(kind: TokenKind): string {
    const token = tokenOf(game, game.currentPlayer, kind)
    const where = lineName(kind, token.line)
    return token.face === 'attack'
      ? `Volley along ${where} first`
      : `Move from ${where} first`
  }

  /** What choosing this order puts in front of the volley, so the choice reads
      without counting squares. The question is only ever put when the volley
      has a shot to take, so there is always something to count. */
  function orderNote(kind: TokenKind): string {
    if (tokenOf(game, game.currentPlayer, kind).face !== 'attack') {
      return 'the volley waits for the piece to land'
    }
    const preview = volley[game.currentPlayer]
    const targets =
      (preview?.threatenedPieces.length ?? 0) +
      (preview?.threatenedCastles.length ?? 0) +
      (preview?.threatenedWalls.length ?? 0)
    return `${targets} ${targets === 1 ? 'target' : 'targets'} in reach now`
  }

  let prompt = $derived.by(() => {
    if (game.outcome) return ''
    if (!isHumanTurn) return `${PLAYER_NAMES[game.currentPlayer]} is deliberating…`
    if (game.phase === 'slide') {
      const movable = choices
        .filter((choice) => choice.type === 'slide')
        .map((choice) => tokenWord(choice.token))
      const unique = [...new Set(movable)]
      return unique.length > 1
        ? 'Opening move: slide either command token to a line holding one of your pieces.'
        : `Slide your ${unique[0]} token to a ${unique[0]} holding one of your pieces.`
    }
    if (game.phase === 'order') return 'Both activations are ready — which fires first?'
    const kind = pendingTokens(game)[0]
    if (!kind) return ''
    const where = lineName(kind, tokenOf(game, game.currentPlayer, kind).line)
    if (game.phase === 'attack') {
      if (!attackerSquare) return ''
      const piece = pieceAt(game, attackerSquare)
      const name = piece ? PIECE_NAMES[piece.kind] : 'attacker'
      return `Volleying along ${where}: your ${name} on ${squareName(attackerSquare)} strikes one target.`
    }
    if (mustPass) return `No piece on ${where} has a legal move.`
    return kind === 'row'
      ? `Move a piece from ${where} like a rook.`
      : `Move a piece from ${where} like a bishop.`
  })

  /** The standing sentence under the prompt: what this decision is really
      deciding. The board replaces it while something is under the cursor. */
  let guidance = $derived.by(() => {
    if (game.outcome || !isHumanTurn) return ''
    if (game.phase === 'slide') {
      return 'The line you choose is also where this token volleys next turn.'
    }
    if (game.phase === 'order') {
      return 'A volley counts only the pieces standing on its line the moment it fires.'
    }
    if (game.phase === 'attack') {
      const left = attackers.length - 1
      return left > 0
        ? `${left} more ${left === 1 ? 'attacker fires' : 'attackers fire'} after this one.`
        : 'The last shot of this volley.'
    }
    if (mustPass) return 'The activation passes; your volley still fires.'
    if (!volleyPending(game)) return 'The volley has fired; this move ends the turn.'
    const armed = pendingTokens(game)[1]
    const where = lineName(armed, tokenOf(game, game.currentPlayer, armed).line)
    return `Your volley along ${where} fires the moment the piece lands.`
  })

  let verdict = $derived.by(() => {
    if (!game.outcome) return null
    if (game.outcome.type === 'draw') return 'A draw — neither side could force a decision.'
    const winner = game.outcome.player
    const loser = 1 - winner
    const reason =
      game.castles[loser].every((standing) => !standing)
        ? 'every castle razed'
        : 'both siege engines destroyed'
    return `${PLAYER_NAMES[winner]} wins — ${PLAYER_NAMES[loser]} has ${reason}.`
  })

  // Only the opening move needs a kick; every turn after is driven from
  // `handleChoice` or from `runAi` itself.
  onMount(advance)
  onDestroy(() => ai.dispose())
</script>

<div class="screen">
  <header class="bar">
    <div class="titles">
      <h1>Command Slide</h1>
      <span class="small-caps">Turn {game.turn + 1} · {setup.difficulty}</span>
    </div>
    <div class="controls">
      <label class="toggle">
        <input type="checkbox" bind:checked={showThreats} />
        Show threats
      </label>
      <label class="toggle">
        <input type="checkbox" bind:checked={showOutcomes} />
        Look ahead
      </label>
      <button onclick={() => (showRules = true)}>Rules</button>
      <button onclick={undo} disabled={history.length === 0}>Undo</button>
      <button onclick={restart}>Restart</button>
      <button onclick={onexit}>Menu</button>
    </div>
  </header>

  <div class="body">
    <aside class="side">
      <PlayerPanel
        {game}
        player={0}
        seat={setup.seats[0] === 'ai' ? 'Machine' : 'You'}
        active={game.currentPlayer === 0 && !game.outcome}
        thinking={thinking && game.currentPlayer === 0}
      />
      <PlayerPanel
        {game}
        player={1}
        seat={setup.seats[1] === 'ai' ? 'Machine' : 'You'}
        active={game.currentPlayer === 1 && !game.outcome}
        thinking={thinking && game.currentPlayer === 1}
      />
      <EventLog {entries} />
    </aside>

    <main class="table">
      <Board
        {game}
        legalChoices={choices}
        {volley}
        moveOutcomes={outcomes}
        slideOutcomes={slides}
        pendingAttackers={attackers}
        {viewer}
        {showThreats}
        {showOutcomes}
        {destroyed}
        onChoice={handleChoice}
        onhint={(text) => (hint = text)}
      />

      <div class="prompt panel">
        {#if verdict}
          <strong class="verdict">{verdict}</strong>
          <div class="actions">
            <button class="primary" onclick={restart}>Play again</button>
            <button onclick={onexit}>Menu</button>
          </div>
        {:else}
          {#if isHumanTurn}
            <ol class="steps small-caps">
              <li class:done={game.phase !== 'slide'} class:now={game.phase === 'slide'}>
                Slide a token
              </li>
              <li class:now={game.phase !== 'slide'}>Activate both</li>
            </ol>
          {/if}
          <span class="line">{prompt}</span>
          {#if hint || guidance}
            <span class="guidance" class:live={hint !== null}>{hint ?? guidance}</span>
          {/if}
          {#if isHumanTurn && game.phase === 'order'}
            <div class="actions">
              {#each orderChoices as choice (choice.type === 'order' ? choice.first : '')}
                {#if choice.type === 'order'}
                  <button class="primary order" onclick={() => handleChoice(choice)}>
                    <span>{orderLabel(choice.first)}</span>
                    <span class="note">{orderNote(choice.first)}</span>
                  </button>
                {/if}
              {/each}
            </div>
          {:else if isHumanTurn && game.phase === 'attack'}
            <div class="actions">
              <button onclick={() => choose('holdFire')}>Hold fire</button>
            </div>
          {:else if isHumanTurn && game.phase === 'activate'}
            <div class="actions">
              <button class:primary={mustPass} onclick={() => choose('pass')}>
                {mustPass ? 'Pass' : 'Take no action'}
              </button>
            </div>
          {/if}
        {/if}
        {#if failure}
          <span class="failure">The opponent could not move: {failure}</span>
        {/if}
      </div>

      <div class="legend small-caps">
        <span><i class="key own"></i> you can shoot</span>
        <span><i class="key own-reach"></i> you cover</span>
        <span><i class="key threat"></i> they can shoot</span>
        <span><i class="key threat-reach"></i> they cover</span>
        <span><i class="key movable"></i> can move</span>
        <span><i class="key dest"></i> destination</span>
        <span><i class="key kill"></i> puts a target in reach</span>
      </div>
    </main>
  </div>
</div>

{#if showRules}
  <RulesOverlay onclose={() => (showRules = false)} />
{/if}

<style>
  .screen {
    min-height: 100dvh;
    display: grid;
    grid-template-rows: auto 1fr;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding: 0.7rem 1.1rem;
    border-bottom: 1px solid var(--gold-soft);
    background: rgba(239, 230, 211, 0.7);
  }

  .titles {
    display: flex;
    align-items: baseline;
    gap: 0.8rem;
  }

  h1 {
    font-size: 1.35rem;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
    color: var(--ink-soft);
    cursor: pointer;
  }

  .body {
    display: grid;
    grid-template-columns: minmax(15rem, 19rem) 1fr;
    gap: 1.2rem;
    padding: 1.1rem;
    align-items: start;
  }

  .side {
    display: grid;
    gap: 0.7rem;
    position: sticky;
    top: 1rem;
  }

  .table {
    display: grid;
    justify-items: center;
    gap: 0.8rem;
    /* The board sizes its cell off this column's width, so the sidebar's
       presence or absence is the only thing that has to be measured. */
    container-type: inline-size;
  }

  .prompt {
    padding: 0.7rem 1rem;
    display: grid;
    gap: 0.6rem;
    justify-items: center;
    text-align: center;
    min-width: min(30rem, 100%);
  }

  .line {
    font-size: 1.02rem;
  }

  .steps {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    font-size: 0.72rem;
    color: var(--ink-faint);
    list-style: none;
  }

  .steps li + li::before {
    content: '→';
    margin-right: 0.5rem;
  }

  .steps li.done {
    text-decoration: line-through;
  }

  .steps li.now {
    color: var(--ink);
    font-weight: 600;
  }

  .guidance {
    font-size: 0.86rem;
    color: var(--ink-soft);
    font-style: italic;
    min-height: 1.2em;
  }

  .guidance.live {
    color: var(--crimson);
    font-style: normal;
  }

  .order {
    display: grid;
    gap: 0.15rem;
    justify-items: center;
    line-height: 1.25;
  }

  .order .note {
    font-size: 0.74rem;
    opacity: 0.75;
    font-weight: 400;
  }

  .verdict {
    font-family: var(--font-display);
    font-size: 1.1rem;
    color: var(--crimson);
  }

  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: center;
  }

  .failure {
    color: var(--crimson);
    font-size: 0.85rem;
  }

  .legend {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    justify-content: center;
  }

  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }

  .key {
    width: 12px;
    height: 12px;
    border-radius: 2px;
    display: inline-block;
  }

  .key.own {
    border: 2px solid var(--gold);
    background: rgba(185, 143, 46, 0.2);
  }

  .key.threat {
    border: 2px solid var(--crimson);
    background: rgba(163, 34, 34, 0.22);
  }

  .key.own-reach {
    background: repeating-linear-gradient(
      -45deg,
      rgba(185, 143, 46, 0.55) 0 2px,
      transparent 2px 6px
    );
    border: 1px solid var(--gold-soft);
  }

  .key.threat-reach {
    background: repeating-linear-gradient(
      45deg,
      rgba(163, 34, 34, 0.5) 0 2px,
      transparent 2px 6px
    );
    border: 1px solid rgba(163, 34, 34, 0.4);
  }

  .key.movable {
    box-shadow: inset 0 0 0 2px var(--gold);
  }

  .key.kill {
    border-radius: 50%;
    background: var(--crimson);
    opacity: 0.7;
  }

  .key.dest {
    border-radius: 50%;
    background: var(--azure);
    opacity: 0.55;
  }

  @media (max-width: 900px) {
    .body {
      grid-template-columns: 1fr;
    }

    .side {
      position: static;
      grid-template-columns: 1fr 1fr;
    }

    .side :global(.log) {
      grid-column: 1 / -1;
    }
  }

  /* On a phone every millimetre of padding comes straight off the board. */
  @media (max-width: 560px) {
    .bar {
      padding: 0.6rem 0.7rem;
    }

    .body {
      gap: 0.8rem;
      padding: 0.6rem;
    }

    .prompt {
      padding: 0.6rem 0.7rem;
    }
  }
</style>
