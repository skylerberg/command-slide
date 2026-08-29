<script lang="ts">
  import PieceIcon from './PieceIcon.svelte'
  import TerrainIcon from './TerrainIcon.svelte'
  import TokenIcon from './TokenIcon.svelte'
  import {
    BOARD_SIZE,
    TOKEN_KINDS,
    attackTargets,
    castleSlotAt,
    isHilltop,
    pieceAt,
    sameSquare,
    squareKey,
    tokenOf,
  } from '../data/types'
  import type { AttackPreview, Choice, GameState, Square, TokenKind } from '../data/types'

  interface Props {
    game: GameState
    /** Empty whenever it is not the local player's move. */
    legalChoices: Choice[]
    /** Squares each side's armed token currently bears on. */
    volley: Record<number, AttackPreview | null>
    /** The seat the human is playing, or null when watching. */
    viewer: number | null
    showThreats: boolean
    /** Squares emptied by the volley just resolved, flashed and then dropped. */
    destroyed: Square[]
    onChoice: (choice: Choice) => void
  }

  let { game, legalChoices, volley, viewer, showThreats, destroyed, onChoice }: Props = $props()

  let selected: Square | null = $state(null)
  let hovered: Square | null = $state(null)

  // A choice landed, so any half-finished selection is about a position that
  // no longer exists.
  $effect(() => {
    void game
    selected = null
  })

  const indices = Array.from({ length: BOARD_SIZE }, (_, i) => i)

  let slideTargets = $derived(
    new Set(
      legalChoices
        .filter((choice) => choice.type === 'slide')
        .map((choice) => `${choice.token}:${choice.line}`),
    ),
  )

  let moves = $derived(legalChoices.filter((choice) => choice.type === 'move'))

  let movable = $derived(new Set(moves.map((choice) => squareKey(choice.from))))

  let destinations = $derived(
    selected === null
      ? new Set<string>()
      : new Set(
          moves
            .filter((choice) => sameSquare(choice.from, selected!))
            .map((choice) => squareKey(choice.to)),
        ),
  )

  /** Where the highlighted piece would strike, shown while learning a pattern. */
  let patternSquares = $derived.by(() => {
    const square = selected ?? hovered
    if (!square) return new Set<string>()
    const piece = pieceAt(game, square)
    if (!piece) return new Set<string>()
    if (piece.kind === 'trebuchet' && !isHilltop(square)) return new Set<string>()
    return new Set(attackTargets(square, piece.kind).map(squareKey))
  })

  function markedSquares(player: number): Set<string> {
    const preview = volley[player]
    if (!preview) return new Set()
    return new Set(
      [...preview.destroyedPieces, ...preview.destroyedCastles].map(squareKey),
    )
  }

  let ownMarks = $derived(viewer === null ? new Set<string>() : markedSquares(viewer))
  let threatMarks = $derived(
    !showThreats || viewer === null ? new Set<string>() : markedSquares(1 - viewer),
  )
  let destroyedKeys = $derived(new Set(destroyed.map(squareKey)))

  /** Lines each side's tokens currently name, for the faint band under them. */
  let bands = $derived.by(() => {
    const rows = new Map<number, string[]>()
    const cols = new Map<number, string[]>()
    for (let player = 0; player < 2; player++) {
      for (const kind of TOKEN_KINDS) {
        const token = tokenOf(game, player, kind)
        const target = kind === 'row' ? rows : cols
        const label = `${player}-${token.face}`
        target.set(token.line, [...(target.get(token.line) ?? []), label])
      }
    }
    return { rows, cols }
  })

  function bandClass(square: Square): string {
    const labels = [
      ...(bands.rows.get(square.row) ?? []),
      ...(bands.cols.get(square.col) ?? []),
    ]
    if (labels.some((label) => label.endsWith('attack'))) return 'band-attack'
    if (labels.length > 0) return 'band-move'
    return ''
  }

  function clickTrack(kind: TokenKind, line: number) {
    if (!slideTargets.has(`${kind}:${line}`)) return
    onChoice({ type: 'slide', token: kind, line })
  }

  function clickSquare(square: Square) {
    if (selected && destinations.has(squareKey(square))) {
      onChoice({ type: 'move', from: selected, to: square })
      return
    }
    if (movable.has(squareKey(square))) {
      selected = selected && sameSquare(selected, square) ? null : square
      return
    }
    selected = null
  }

  function trackActive(player: number, kind: TokenKind, line: number): boolean {
    return viewer === player && slideTargets.has(`${kind}:${line}`)
  }

  function trackHasToken(player: number, kind: TokenKind, line: number): boolean {
    return tokenOf(game, player, kind).line === line
  }

  function squareTitle(square: Square): string {
    const piece = pieceAt(game, square)
    const parts: string[] = [`${'abcdefg'[square.col]}${BOARD_SIZE - square.row}`]
    if (piece) parts.push(piece.kind)
    if (isHilltop(square)) parts.push('hilltop')
    const slot = castleSlotAt(square)
    if (slot) parts.push(game.castles[slot.owner][slot.index] ? 'castle' : 'ruined castle')
    return parts.join(' · ')
  }
</script>

<div class="board-frame">
  <div class="grid">
    <div class="corner"></div>
    {#each indices as col (col)}
      <button
        class="track column"
        class:live={trackActive(0, 'column', col)}
        class:occupied={trackHasToken(0, 'column', col)}
        disabled={!trackActive(0, 'column', col)}
        onclick={() => clickTrack('column', col)}
        aria-label={`Ivory column token to column ${col + 1}`}
      >
        {#if trackHasToken(0, 'column', col)}
          <span class="token ivory">
            <TokenIcon kind="column" face={tokenOf(game, 0, 'column').face} owner={0} />
          </span>
        {:else}
          <span class="slot"></span>
        {/if}
      </button>
    {/each}
    <div class="corner"></div>

    {#each indices as row (row)}
      <button
        class="track row"
        class:live={trackActive(0, 'row', row)}
        class:occupied={trackHasToken(0, 'row', row)}
        disabled={!trackActive(0, 'row', row)}
        onclick={() => clickTrack('row', row)}
        aria-label={`Ivory row token to row ${row + 1}`}
      >
        {#if trackHasToken(0, 'row', row)}
          <span class="token ivory">
            <TokenIcon kind="row" face={tokenOf(game, 0, 'row').face} owner={0} />
          </span>
        {:else}
          <span class="slot"></span>
        {/if}
      </button>

      {#each indices as col (col)}
        {@const square = { row, col }}
        {@const key = squareKey(square)}
        {@const piece = pieceAt(game, square)}
        {@const slot = castleSlotAt(square)}
        <button
          class={`square ${bandClass(square)}`}
          class:castle-square={slot !== null && game.castles[slot.owner][slot.index]}
          class:castle-razed={slot !== null && !game.castles[slot.owner][slot.index]}
          class:hilltop={isHilltop(square)}
          class:armed-siege={piece !== null &&
            piece.kind === 'trebuchet' &&
            isHilltop(square)}
          class:selected={selected !== null && sameSquare(selected, square)}
          class:movable={movable.has(key)}
          class:destination={destinations.has(key)}
          class:pattern={patternSquares.has(key)}
          class:own-mark={ownMarks.has(key)}
          class:threat={threatMarks.has(key)}
          class:flash={destroyedKeys.has(key)}
          onclick={() => clickSquare(square)}
          onmouseenter={() => (hovered = square)}
          onmouseleave={() => (hovered = null)}
          onfocus={() => (hovered = square)}
          onblur={() => (hovered = null)}
          title={squareTitle(square)}
        >
          <span class="terrain">
            {#if slot}
              <span class={game.castles[slot.owner][slot.index] ? 'castle' : 'ruin'}>
                <TerrainIcon
                  kind={game.castles[slot.owner][slot.index] ? 'castle' : 'ruin'}
                  size={40}
                />
              </span>
            {:else if isHilltop(square)}
              <span class="hill"><TerrainIcon kind="hilltop" size={34} /></span>
            {/if}
          </span>

          {#if piece}
            <span class={`token piece ${piece.owner === 0 ? 'ivory' : 'umber'}`}>
              <PieceIcon kind={piece.kind} owner={piece.owner} />
            </span>
          {/if}

          <span class="overlay"></span>
        </button>
      {/each}

      <button
        class="track row"
        class:live={trackActive(1, 'row', row)}
        class:occupied={trackHasToken(1, 'row', row)}
        disabled={!trackActive(1, 'row', row)}
        onclick={() => clickTrack('row', row)}
        aria-label={`Umber row token to row ${row + 1}`}
      >
        {#if trackHasToken(1, 'row', row)}
          <span class="token umber">
            <TokenIcon kind="row" face={tokenOf(game, 1, 'row').face} owner={1} />
          </span>
        {:else}
          <span class="slot"></span>
        {/if}
      </button>
    {/each}

    <div class="corner"></div>
    {#each indices as col (col)}
      <button
        class="track column"
        class:live={trackActive(1, 'column', col)}
        class:occupied={trackHasToken(1, 'column', col)}
        disabled={!trackActive(1, 'column', col)}
        onclick={() => clickTrack('column', col)}
        aria-label={`Umber column token to column ${col + 1}`}
      >
        {#if trackHasToken(1, 'column', col)}
          <span class="token umber">
            <TokenIcon kind="column" face={tokenOf(game, 1, 'column').face} owner={1} />
          </span>
        {:else}
          <span class="slot"></span>
        {/if}
      </button>
    {/each}
    <div class="corner"></div>
  </div>
</div>

<style>
  .board-frame {
    --cell: clamp(34px, 8.4vmin, 72px);
    --track: calc(var(--cell) * 0.72);
    display: inline-block;
    padding: 0.4rem;
  }

  .grid {
    display: grid;
    grid-template-columns: var(--track) repeat(7, var(--cell)) var(--track);
    grid-template-rows: var(--track) repeat(7, var(--cell)) var(--track);
  }

  .corner {
    width: var(--track);
    height: var(--track);
  }

  .track {
    display: grid;
    place-items: center;
    padding: 0;
    background: none;
    border: none;
    border-radius: 50%;
    cursor: default;
  }

  .track:hover:not(:disabled) {
    background: none;
    transform: none;
  }

  .slot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--ink-faint);
    opacity: 0.5;
  }

  /* A token never slides to the line it already holds, so its own track is
     always disabled. The art still has to read at full strength. */
  .track:disabled {
    opacity: 1;
  }

  .track.live {
    cursor: pointer;
  }

  .track.live .slot {
    width: 60%;
    height: 60%;
    background: transparent;
    border: 2px dashed var(--gold);
    opacity: 0.85;
  }

  .track.live:hover .slot {
    background: var(--gold-soft);
    border-style: solid;
  }

  .token {
    display: grid;
    place-items: center;
    width: 88%;
    aspect-ratio: 1;
    border-radius: 50%;
    box-shadow: var(--shadow-soft);
  }

  /* The art is a disc of the owner's colour, edge to edge. The matching
     background only guards the seam where the CSS circle meets the drawn one. */
  .token.ivory {
    background: var(--ivory);
  }

  .token.umber {
    background: var(--umber);
  }

  .track .token {
    width: 84%;
  }

  .square {
    position: relative;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1px solid var(--board-line);
    border-radius: 0;
    background: rgba(255, 253, 246, 0.55);
    cursor: default;
    transition: background 0.12s;
  }

  .square:hover:not(:disabled) {
    transform: none;
    border-color: var(--board-line);
  }

  .square.band-move {
    background: rgba(43, 110, 168, 0.13);
  }

  .square.band-attack {
    background: rgba(163, 34, 34, 0.14);
  }

  /* Castle squares stay legible under a piece: the terrain tint carries them
     even when the icon is mostly covered. */
  .square.castle-square {
    background: var(--castle-tint);
    box-shadow: inset 0 0 0 2px rgba(122, 90, 51, 0.45);
  }

  .square.castle-razed {
    background: repeating-linear-gradient(
      45deg,
      rgba(122, 90, 51, 0.1) 0 4px,
      transparent 4px 8px
    );
  }

  .square.hilltop {
    background: var(--hilltop);
  }

  .terrain {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  .castle {
    color: var(--ink);
    opacity: 0.55;
  }

  .ruin {
    color: var(--ink-faint);
    opacity: 0.6;
  }

  .hill {
    color: var(--ink-soft);
    opacity: 0.6;
  }

  .piece {
    position: relative;
    z-index: 2;
    width: 82%;
  }

  .square.armed-siege .piece {
    box-shadow:
      0 0 0 3px var(--gold),
      0 0 10px rgba(185, 143, 46, 0.7);
  }

  .overlay {
    position: absolute;
    inset: 0;
    z-index: 3;
    pointer-events: none;
  }

  /* A piece this activation could move. */
  .square.movable {
    cursor: pointer;
  }

  .square.movable .overlay {
    box-shadow: inset 0 0 0 2px var(--gold);
  }

  .square.selected .overlay {
    box-shadow: inset 0 0 0 3px var(--amber);
    background: rgba(208, 115, 26, 0.12);
  }

  .square.destination {
    cursor: pointer;
  }

  .square.destination .overlay::after {
    content: '';
    position: absolute;
    inset: 0;
    margin: auto;
    width: 28%;
    height: 28%;
    border-radius: 50%;
    background: var(--azure);
    opacity: 0.55;
  }

  /* Where the piece under the cursor would strike. */
  .square.pattern .overlay {
    box-shadow: inset 0 0 0 2px rgba(163, 34, 34, 0.45);
  }

  /* Something your armed token would destroy right now. */
  .square.own-mark .overlay::before {
    content: '';
    position: absolute;
    inset: 6%;
    border: 2px solid var(--gold);
    border-radius: 3px;
    background: rgba(185, 143, 46, 0.2);
  }

  /* Something the opponent's armed token would destroy right now. */
  .square.threat .overlay::before {
    content: '';
    position: absolute;
    inset: 6%;
    border: 2px solid var(--crimson);
    border-radius: 3px;
    background: rgba(163, 34, 34, 0.22);
  }

  .square.flash .overlay {
    animation: strike 0.7s ease-out;
  }

  @keyframes strike {
    0% {
      background: rgba(163, 34, 34, 0.75);
    }
    100% {
      background: transparent;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .square.flash .overlay {
      animation: none;
      background: rgba(163, 34, 34, 0.25);
    }
  }
</style>
