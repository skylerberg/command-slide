<script lang="ts">
  import PieceIcon from './PieceIcon.svelte'
  import TerrainIcon from './TerrainIcon.svelte'
  import TokenIcon from './TokenIcon.svelte'
  import { describeMoveOutcome, describeSlideOutcome } from '../data/log'
  import {
    BOARD_COLS,
    BOARD_ROWS,
    FILE_LETTERS,
    TOKEN_KINDS,
    attackTargets,
    castleSlotAt,
    isHilltop,
    lineOf,
    pieceAt,
    sameSquare,
    squareKey,
    tokenOf,
    wallStands,
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

  interface Props {
    game: GameState
    /** Empty whenever it is not the local player's move. */
    legalChoices: Choice[]
    /** Squares each side's armed token currently bears on. */
    volley: Record<number, AttackPreview | null>
    /** What each move available now puts in reach; empty off the move phase. */
    moveOutcomes: MoveOutcome[]
    /** What each slide available now sets up; empty off the slide phase. */
    slideOutcomes: SlideOutcome[]
    /** Attackers of the volley in progress still to fire, the current one first. */
    pendingAttackers: Square[]
    /** The seat the human is playing, or null when watching. */
    viewer: number | null
    showThreats: boolean
    /** Whether to look a step ahead: what a move or a slide would lead to. */
    showOutcomes: boolean
    /** Squares emptied by the volley just resolved, flashed and then dropped. */
    destroyed: Square[]
    onChoice: (choice: Choice) => void
    /** A sentence about whatever is under the cursor, for the prompt panel. */
    onhint: (hint: string | null) => void
  }

  let {
    game,
    legalChoices,
    volley,
    moveOutcomes,
    slideOutcomes,
    pendingAttackers,
    viewer,
    showThreats,
    showOutcomes,
    destroyed,
    onChoice,
    onhint,
  }: Props = $props()

  let selected: Square | null = $state(null)
  let hovered: Square | null = $state(null)
  let hoveredSlide: SlideOutcome | null = $state(null)

  const rows = Array.from({ length: BOARD_ROWS }, (_, i) => i)
  const cols = Array.from({ length: BOARD_COLS }, (_, i) => i)

  let moves = $derived(legalChoices.filter((choice) => choice.type === 'move'))

  let movable = $derived(new Set(moves.map((choice) => squareKey(choice.from))))

  // A choice landed, so any half-finished selection is about a position that no
  // longer exists. When one piece is the only one that can move there is
  // nothing to pick: select it so its destinations are already on the board.
  $effect(() => {
    void game
    selected = movable.size === 1 ? moves[0].from : null
    hovered = null
    hoveredSlide = null
  })

  let slideTargets = $derived(
    new Map(slideOutcomes.map((outcome) => [`${outcome.token}:${outcome.line}`, outcome])),
  )

  /** The one piece now choosing a target, while a volley walks its line. */
  let attacker = $derived.by(() => {
    for (const choice of legalChoices) {
      if (choice.type === 'attack' || choice.type === 'holdFire') return choice.from
    }
    return null
  })

  let targets = $derived(
    new Set(
      legalChoices
        .filter((choice) => choice.type === 'attack')
        .map((choice) => squareKey(choice.target)),
    ),
  )

  let destinations = $derived(
    selected === null
      ? new Set<string>()
      : new Set(
          moves
            .filter((choice) => sameSquare(choice.from, selected!))
            .map((choice) => squareKey(choice.to)),
        ),
  )

  /** Everything one volley could shoot: pieces, castles and now walls too. */
  function threatened(reach: {
    threatenedPieces: Square[]
    threatenedCastles: Square[]
    threatenedWalls: Square[]
  }): Square[] {
    return [...reach.threatenedPieces, ...reach.threatenedCastles, ...reach.threatenedWalls]
  }

  function targeting(outcome: MoveOutcome): boolean {
    return threatened(outcome).length > 0
  }

  /** The move to each destination of the selected piece, keyed by destination. */
  let outcomeByDestination = $derived.by(() => {
    const map = new Map<string, MoveOutcome>()
    if (!selected) return map
    for (const outcome of moveOutcomes) {
      if (sameSquare(outcome.from, selected)) map.set(squareKey(outcome.to), outcome)
    }
    return map
  })

  /** Pieces holding a move that would put something under the volley. */
  let armingMovers = $derived(
    new Set(
      showOutcomes ? moveOutcomes.filter(targeting).map((o) => squareKey(o.from)) : [],
    ),
  )

  /** What the destination under the cursor would bring within reach. */
  let inReach = $derived.by(() => {
    if (!showOutcomes || !hovered) return new Set<string>()
    const outcome = outcomeByDestination.get(squareKey(hovered))
    if (!outcome) return new Set<string>()
    return keys(threatened(outcome))
  })

  /** Where the highlighted piece strikes — from where it would land, if the
      cursor is on one of its destinations. */
  let patternSquares = $derived.by(() => {
    const landing = hovered && destinations.has(squareKey(hovered)) ? hovered : null
    const origin = landing ?? selected ?? hovered
    if (!origin) return new Set<string>()
    const piece = pieceAt(game, landing ? selected! : origin)
    if (!piece) return new Set<string>()
    if (piece.kind === 'trebuchet' && !isHilltop(game, origin)) return new Set<string>()
    return new Set(attackTargets(origin, piece.kind).map(squareKey))
  })

  function keys(squares: Square[]): Set<string> {
    return new Set(squares.map(squareKey))
  }

  /** The volley the board is explaining: the armed token's, or the one a
      hovered slide would arm a turn from now. Suppressed while an attacker is
      choosing its target, where the legal shots say it better. */
  let ownVolley = $derived.by(() => {
    if (hoveredSlide && showOutcomes) {
      return {
        covered: keys(hoveredSlide.covered),
        marks: keys(threatened(hoveredSlide)),
      }
    }
    const preview = viewer === null || attacker !== null ? null : volley[viewer]
    return {
      covered: keys(preview?.covered ?? []),
      marks: preview ? keys(threatened(preview)) : new Set<string>(),
    }
  })

  let enemyVolley = $derived.by(() => {
    const preview = !showThreats || viewer === null ? null : volley[1 - viewer]
    return {
      covered: keys(preview?.covered ?? []),
      marks: preview ? keys(threatened(preview)) : new Set<string>(),
    }
  })

  /** Attackers behind the one now firing: the shots still to come. */
  let queuedAttackers = $derived(keys(pendingAttackers.slice(1)))
  let previewMovers = $derived.by(() => keys(hoveredSlide?.movers ?? []))
  let destroyedKeys = $derived(keys(destroyed))

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

  /** An armed line reads by whose it is: the one you are about to be shot
      along is not the one you are about to shoot along. */
  function bandClass(square: Square): string {
    const labels = [
      ...(bands.rows.get(square.row) ?? []),
      ...(bands.cols.get(square.col) ?? []),
    ]
    const mine = viewer ?? 0
    if (labels.includes(`${1 - mine}-attack`)) return 'band-threat'
    if (labels.includes(`${mine}-attack`)) return 'band-armed'
    if (labels.length > 0) return 'band-move'
    return ''
  }

  function onPreviewedLine(square: Square): boolean {
    return hoveredSlide !== null && lineOf(hoveredSlide.token, square) === hoveredSlide.line
  }

  function clickTrack(kind: TokenKind, line: number) {
    if (!slideTargets.has(`${kind}:${line}`)) return
    onChoice({ type: 'slide', token: kind, line })
  }

  function enterTrack(kind: TokenKind, line: number) {
    const outcome = slideTargets.get(`${kind}:${line}`)
    if (!outcome) return
    hoveredSlide = outcome
    onhint(showOutcomes ? describeSlideOutcome(game, outcome) : null)
  }

  function leaveTrack() {
    hoveredSlide = null
    onhint(null)
  }

  function enterSquare(square: Square) {
    hovered = square
    const outcome = outcomeByDestination.get(squareKey(square))
    onhint(outcome && showOutcomes ? describeMoveOutcome(game, outcome) : null)
  }

  function leaveSquare() {
    hovered = null
    onhint(null)
  }

  function clickSquare(square: Square) {
    if (attacker && targets.has(squareKey(square))) {
      onChoice({ type: 'attack', from: attacker, target: square })
      return
    }
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
    const parts: string[] = [`${FILE_LETTERS[square.col]}${BOARD_ROWS - square.row}`]
    if (piece) parts.push(piece.kind)
    const slot = castleSlotAt(square)
    if (slot) parts.push(game.castles[slot.owner][slot.index] ? 'castle' : 'razed castle')
    if (wallStands(game, square)) parts.push('wall')
    if (isHilltop(game, square)) parts.push('hilltop')
    return parts.join(' · ')
  }
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === 'Escape') selected = null
  }}
/>

<div class="board-frame">
  <div class="grid">
    <div class="corner"></div>
    {#each cols as col (col)}
      <button
        class="track column"
        class:live={trackActive(0, 'column', col)}
        class:occupied={trackHasToken(0, 'column', col)}
        disabled={!trackActive(0, 'column', col)}
        onclick={() => clickTrack('column', col)}
        onmouseenter={() => enterTrack('column', col)}
        onmouseleave={leaveTrack}
        onfocus={() => enterTrack('column', col)}
        onblur={leaveTrack}
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

    {#each rows as row (row)}
      <button
        class="track row"
        class:live={trackActive(0, 'row', row)}
        class:occupied={trackHasToken(0, 'row', row)}
        disabled={!trackActive(0, 'row', row)}
        onclick={() => clickTrack('row', row)}
        onmouseenter={() => enterTrack('row', row)}
        onmouseleave={leaveTrack}
        onfocus={() => enterTrack('row', row)}
        onblur={leaveTrack}
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

      {#each cols as col (col)}
        {@const square = { row, col }}
        {@const key = squareKey(square)}
        {@const piece = pieceAt(game, square)}
        {@const slot = castleSlotAt(square)}
        {@const outcome = outcomeByDestination.get(key)}
        <button
          class={`square ${bandClass(square)}`}
          class:castle-square={slot !== null && game.castles[slot.owner][slot.index]}
          class:castle-razed={slot !== null && !game.castles[slot.owner][slot.index]}
          class:hilltop={isHilltop(game, square)}
          class:wall={wallStands(game, square)}
          class:armed-siege={piece !== null &&
            piece.kind === 'trebuchet' &&
            isHilltop(game, square)}
          class:line-preview={onPreviewedLine(square)}
          class:preview-mover={previewMovers.has(key)}
          class:selected={selected !== null && sameSquare(selected, square)}
          class:firing={attacker !== null && sameSquare(attacker, square)}
          class:queued={queuedAttackers.has(key)}
          class:target={targets.has(key)}
          class:movable={movable.has(key)}
          class:arming={armingMovers.has(key)}
          class:destination={destinations.has(key)}
          class:aims={showOutcomes && outcome !== undefined && targeting(outcome)}
          class:pattern={patternSquares.has(key)}
          class:covered-own={ownVolley.covered.has(key)}
          class:covered-threat={enemyVolley.covered.has(key)}
          class:own-mark={ownVolley.marks.has(key)}
          class:threat={enemyVolley.marks.has(key)}
          class:in-reach={inReach.has(key)}
          class:flash={destroyedKeys.has(key)}
          onclick={() => clickSquare(square)}
          onmouseenter={() => enterSquare(square)}
          onmouseleave={leaveSquare}
          onfocus={() => enterSquare(square)}
          onblur={leaveSquare}
          title={squareTitle(square)}
        >
          <span class="terrain">
            {#if slot && game.castles[slot.owner][slot.index]}
              <span class="castle"><TerrainIcon kind="castle" /></span>
            {:else if wallStands(game, square)}
              <span class="rampart"><TerrainIcon kind="wall" /></span>
            {:else if isHilltop(game, square)}
              <span class="hill"><TerrainIcon kind="hilltop" /></span>
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
        onmouseenter={() => enterTrack('row', row)}
        onmouseleave={leaveTrack}
        onfocus={() => enterTrack('row', row)}
        onblur={leaveTrack}
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
    {#each cols as col (col)}
      <button
        class="track column"
        class:live={trackActive(1, 'column', col)}
        class:occupied={trackHasToken(1, 'column', col)}
        disabled={!trackActive(1, 'column', col)}
        onclick={() => clickTrack('column', col)}
        onmouseenter={() => enterTrack('column', col)}
        onmouseleave={leaveTrack}
        onfocus={() => enterTrack('column', col)}
        onblur={leaveTrack}
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
    --pad: 0.4rem;
    /* Nine files plus a token track each side, at 0.72 of a cell: the grid is
       this many cells wide, and the cell is the width divided by it. */
    --spans: 10.44;
    /* The board answers to the space it is actually in, not to the window:
       `cqw` is measured against the containing column, so the same rule fills
       a phone edge to edge and leaves the desktop sidebar alone. Height still
       has a say, for a window that is wide and short. */
    --cell: clamp(
      26px,
      min(9vh, calc((100cqw - 2 * var(--pad)) / var(--spans))),
      68px
    );
    --track: calc(var(--cell) * 0.72);
    display: inline-block;
    padding: var(--pad);
  }

  .grid {
    display: grid;
    grid-template-columns: var(--track) repeat(9, var(--cell)) var(--track);
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
    width: calc(var(--track) * 0.6);
    height: calc(var(--track) * 0.6);
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

  /* The track cells are oblong, so a percentage would size the token off the
     long side and spill it onto the board. */
  .track .token {
    width: calc(var(--track) * 0.84);
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

  .square.band-armed {
    background: rgba(185, 143, 46, 0.16);
  }

  .square.band-threat {
    background: rgba(163, 34, 34, 0.13);
  }

  /* Castle squares stay legible under a piece: the terrain tint carries them
     even when the icon is mostly covered. */
  .square.castle-square {
    background: var(--castle-tint);
    box-shadow: inset 0 0 0 2px rgba(122, 90, 51, 0.45);
  }

  .square.hilltop {
    background: var(--hilltop);
  }

  /* The line a hovered slide destination would name. Amber is the colour of
     the thing you are about to pick, as it is for a selected piece. */
  .square.line-preview {
    background: rgba(208, 115, 26, 0.2);
  }

  /* Neutral ground nobody owns: no piece may enter it while it stands, and any
     ordinary piece breaks it in one shot. */
  .square.wall {
    background: var(--parchment-deep);
  }

  .rampart {
    width: 59%;
    color: var(--umber-edge);
    opacity: 0.9;
  }

  /* A razed castle takes a hilltop token; the hatch keeps what it was legible. */
  .square.castle-razed {
    background:
      repeating-linear-gradient(45deg, rgba(122, 90, 51, 0.16) 0 4px, transparent 4px 8px),
      var(--hilltop);
  }

  .terrain {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    pointer-events: none;
  }

  /* Terrain is drawn as a share of the cell: a fixed size spills off the
     square once the board shrinks to a phone. */
  .castle,
  .rampart,
  .hill {
    display: block;
    aspect-ratio: 1;
  }

  .castle {
    width: 59%;
    color: var(--ink);
    opacity: 0.55;
  }

  .hill {
    width: 50%;
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

  /* Where a volley lands, whether or not anything is standing there. Gold is
     yours, crimson theirs; a square in both hatches is a trade. */
  .square.covered-own .overlay {
    background-image: repeating-linear-gradient(
      -45deg,
      rgba(185, 143, 46, 0.28) 0 2px,
      transparent 2px 7px
    );
  }

  .square.covered-threat .overlay {
    background-image: repeating-linear-gradient(
      45deg,
      rgba(163, 34, 34, 0.26) 0 2px,
      transparent 2px 7px
    );
  }

  .square.covered-own.covered-threat .overlay {
    background-image:
      repeating-linear-gradient(-45deg, rgba(185, 143, 46, 0.28) 0 2px, transparent 2px 7px),
      repeating-linear-gradient(45deg, rgba(163, 34, 34, 0.26) 0 2px, transparent 2px 7px);
  }

  /* A piece this activation could move. */
  .square.movable {
    cursor: pointer;
  }

  /* Pieces on the line a hovered slide would name. */
  .square.preview-mover .overlay {
    box-shadow: inset 0 0 0 2px var(--gold);
  }

  .square.movable .overlay {
    box-shadow: inset 0 0 0 2px var(--gold);
  }

  .square.selected .overlay {
    box-shadow: inset 0 0 0 3px var(--amber);
    background-color: rgba(208, 115, 26, 0.12);
  }

  /* A piece holding a move that would put something under the volley. */
  .square.arming .overlay::after {
    content: '';
    position: absolute;
    top: 6%;
    right: 6%;
    width: 22%;
    height: 22%;
    border-radius: 50%;
    background: var(--crimson);
    opacity: 0.75;
  }

  /* The attacker now choosing its one target. */
  .square.firing .overlay {
    box-shadow: inset 0 0 0 3px var(--amber);
    background-color: rgba(208, 115, 26, 0.12);
  }

  /* Attackers behind it: this volley still has their shots to take. */
  .square.queued .overlay {
    box-shadow: inset 0 0 0 2px rgba(208, 115, 26, 0.45);
  }

  .square.target {
    cursor: pointer;
  }

  .square.target .overlay {
    box-shadow: inset 0 0 0 3px var(--crimson);
    background-color: rgba(163, 34, 34, 0.22);
  }

  .square.target:hover .overlay {
    background-color: rgba(163, 34, 34, 0.38);
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

  /* A destination that would bring something under the volley. */
  .square.destination.aims .overlay::after {
    width: 38%;
    height: 38%;
    background: var(--crimson);
    opacity: 0.7;
    box-shadow: 0 0 0 3px rgba(163, 34, 34, 0.25);
  }

  /* Where the piece under the cursor would strike. */
  .square.pattern .overlay {
    box-shadow: inset 0 0 0 2px rgba(163, 34, 34, 0.45);
  }

  /* Something your armed token bears on. */
  .square.own-mark .overlay::before {
    content: '';
    position: absolute;
    inset: 6%;
    border: 2px solid var(--gold);
    border-radius: 3px;
    background: rgba(185, 143, 46, 0.2);
  }

  /* Something the opponent's armed token bears on. */
  .square.threat .overlay::before {
    content: '';
    position: absolute;
    inset: 6%;
    border: 2px solid var(--crimson);
    border-radius: 3px;
    background: rgba(163, 34, 34, 0.22);
  }

  /* What the destination under the cursor would put in reach. Last, so it
     wins over the softer marks it sits on top of. */
  .square.in-reach .overlay {
    box-shadow: inset 0 0 0 3px var(--crimson);
  }

  .square.in-reach .overlay::before {
    content: '';
    position: absolute;
    inset: 6%;
    border: 2px solid var(--crimson);
    border-radius: 3px;
    background: rgba(163, 34, 34, 0.35);
  }

  .square.flash .overlay {
    animation: strike 0.7s ease-out;
  }

  @keyframes strike {
    0% {
      background-color: rgba(163, 34, 34, 0.75);
    }
    100% {
      background-color: transparent;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .square.flash .overlay {
      animation: none;
      background-color: rgba(163, 34, 34, 0.25);
    }
  }
</style>
