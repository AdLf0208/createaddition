#!/usr/bin/env bash
# In-game check for Create Crafts & Additions' FE half: two connectors, one wire
# and two modular accumulators, with the jar's own rates.
#
# Every mod command is typed with `ctl chat "/cmd"` because `ctl command` runs
# the console path and bypasses the native `perform_command` hook.
#
# The End, in the void at x=200 (away from the central island, its dragon egg
# and the ambient item entities of a generated level), on a stone floor.
#
# Arena (all at y=100 unless said):
#   A  accumulator (source)   x=10      C1 small connector x=11 facing west
#   C2 small connector x=15 facing east   B accumulator (sink) x=16
# so C1 is mounted on A and C2 on B, and the wire spans four blocks.
#
# Default run:
#  A  a connector placed beside an accumulator takes `pull` by itself, and the
#     accumulator is 2,000,000 FE; the connector's window is 1,000 FE;
#  B  a source accumulator seeded with 100,000 FE sends energy across the wire
#     to the sink through a `push` connector: the source loses exactly what the
#     sink gains plus what the network still holds (FE is moved, never copied),
#     and no tick moves more than the small connector's 1,000 FE;
#  C  the CONTROL: with the wire taken out, nothing moves at all over the same
#     number of ticks — a connector with no wire is not a network;
#  D  a wire stretched past a small connector's 16 blocks is refused, and no
#     link is recorded;
#  E  copper between two large connectors is refused (gold is accepted), as
#     upstream's `REQUIRES_HIGH_CURRENT`.
#
# Default run (EMBEDDED=0): the game loads `content/` as a dev mod.
# EMBEDDED=1: the game is launched with **only the native bundle** (no
# `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/<id>/data/…`, the AE2 pattern). The run also checks the file the
# helper wrote, and CONTROL=1 (default with EMBEDDED) first launches the same
# binary with `.horizon-cache` unusable, where the ids must be unknown.
#
# SELFTEST=1 flips B's conservation assertion (it must FAIL), proving the
# scenario can fail.
set -Eeuo pipefail
export LC_ALL=C
[ -z "${CA_DEBUG:-}" ] || set -x

EMBEDDED=${EMBEDDED:-0}
CONTROL=${CONTROL:-$EMBEDDED}
SELFTEST=${SELFTEST:-0}

CLONE=${CLONE:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/createaddition
PIN_BIN=$CLONE/ports/_shared/bin/horizon-1d63e0a5
CTL=${CTL:-$PIN_BIN/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/createaddition-e2e.sock}
KEEP=${KEEP:-0}
WORLD=CreateAdditionE2E
CONTROL_WORLD=CreateAdditionControl
# The arena: the End's void, far from the central island.
X0=200
AX=$((X0 - 10))
CX1=$((AX + 1))
CX2=$((CX1 + 4))
B=$((CX2 + 1))
# The long-wire pair: 17 blocks apart, past a small connector's 16.
FAR_A=$((X0 + 2))
FAR_B=$((FAR_A + 17))
# The large-connector pair: four blocks apart.
BIG_A=$((X0 + 6))
BIG_B=$((BIG_A + 4))
TICKS=20
RATE=1000
SEED=100000
CAPACITY=2000000

command -v jq >/dev/null || { echo "missing jq (every assertion parses a ctl reply)" >&2; exit 1; }
[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/createaddition-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }
on_error() { echo "FAIL: line $1: '$2' exited $?" >&2; ctl logs --lines 40 >&2 || true; }
trap 'on_error "$LINENO" "$BASH_COMMAND"' ERR

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    # Last line of every run: `check-e2e-logs.py` reads it as the verdict.
    echo "EXIT:$status"
}
trap cleanup EXIT

launch() { # launch [WORLD] [extra args]
    local world=$1; shift
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless
                --wait --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" "$@" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

block_name() { ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty'; }

# The End's void at X0: a stone floor and clear air above it.
arena() {
    ctl command "execute in minecraft:the_end run tp @s $X0.5 104 0.5" >/dev/null
    local loaded=false
    for _ in $(seq 1 120); do
        ctl wait 5 >/dev/null
        if [ "$(ctl block "$X0" 100 0 | jq -r '.loaded // false')" = true ]; then
            loaded=true
            break
        fi
    done
    [ "$loaded" = true ] || fail "the End arena chunk never loaded"
    ctl command "execute in minecraft:the_end run fill $((X0 - 20)) 100 -14 $((X0 + 30)) 100 14 minecraft:stone" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run fill $((X0 - 20)) 101 -14 $((X0 + 30)) 108 14 minecraft:air" >/dev/null
    ctl wait 6 >/dev/null
    ctl command "execute in minecraft:the_end run kill @e[type=!player]" >/dev/null
    ctl command "execute in minecraft:the_end run tp @s $X0.5 101 0.5" >/dev/null
    ctl wait 20
    [ "$(block_name "$X0" 100 0)" = "minecraft:stone" ] || fail "the arena floor is not stone"
    [ "$(block_name "$X0" 101 0)" = "minecraft:air" ] || fail "the arena air above the floor is not air"
}

# Every reply is tagged: the command carries a `#tag` word and the mod appends
# ` [#tag]` to its answer, so a reply from an earlier command (chat lines can
# arrive twice) can never be read as this command's answer.
LINK_REPLY='^(Nodes linked\.|The connectors are too far apart for this wire\.|Every node of this connector has a wire\.|These connectors are already linked\.|A connector cannot be linked to itself\.|Two large connectors need a gold or electrum wire\.)'
WIRE_REPLY='^(Wire removed\.|These connectors are not linked\.)'
MODE_REPLY='^Mode set to (none|pull|push)\.'
ENERGY_REPLY='^Energy set to [0-9]+ FE\.'
MODE_REFUSAL='^The mode is none, pull or push\.'

NEXT_TAG=0

chat() { ctl messages 60 | jq -r '.chat[]'; }

# Type a native command with a fresh tag appended and wait for its answer; the
# tag is removed from REPLY.
command_reply() { # command_reply COMMAND REPLY_PATTERN
    NEXT_TAG=$((NEXT_TAG + 1))
    local tag pattern
    tag="q$NEXT_TAG"
    pattern="$2 .*\\[#$tag\\]$"
    say "$1 #$tag"
    for _ in $(seq 1 30); do
        ctl wait 3 >/dev/null
        if chat | grep -qE "$pattern"; then
            REPLY=$(chat | grep -E "$pattern" | tail -1)
            REPLY=${REPLY% "[#$tag]"}
            REPLY=${REPLY% }
            return 0
        fi
    done
    echo "--- newest chat lines:" >&2
    chat | tail -15 >&2
    fail "no reply to /$1 #$tag"
}

# `/createaddition info X Y Z`: the line, then the parsed fields.
info() { # info X Y Z
    # The reply names the position, so a line about another block can never be
    # mistaken for this one.
    command_reply "createaddition info $1 $2 $3" "^(Connector|Large Connector|Modular Accumulator) at $1,$2,$3:"
    INFO_LINE=$REPLY
    eval "$(python3 "$PORT/tools/parse_info.py" "$INFO_LINE")"
    [ "${ok:-0}" = 1 ] || fail "the info line did not parse: $INFO_LINE"
    echo "  $INFO_LINE" >&2
}

# One connector and one accumulator on the floor, then the connector settles.
place_connector() { # place_connector X FACING [BLOCK] [Y]
    local x=$1 facing=$2 block=${3:-createaddition:connector} y=${4:-100}
    ctl command "execute in minecraft:the_end run setblock $x $y 0 $block[facing=$facing,mode=none]" >/dev/null
    ctl wait 6 >/dev/null
    [ "$(block_name "$x" "$y" 0)" = "$block" ] \
        || fail "the $block did not place at $x,$y (got '$(block_name "$x" "$y" 0)')"
}

place_accumulator() { # place_accumulator X [Y]
    local y=${2:-100}
    ctl command "execute in minecraft:the_end run setblock $1 $y 0 createaddition:modular_accumulator" >/dev/null
    ctl wait 6 >/dev/null
    [ "$(block_name "$1" "$y" 0)" = "createaddition:modular_accumulator" ] \
        || fail "the accumulator did not place at $1,$y"
}

# `/createaddition energy X Y Z AMOUNT`.
seed_at() { # seed_at X Y AMOUNT
    command_reply "createaddition energy $1 $2 0 $3" "$ENERGY_REPLY"
}

# The source's charge, the sink's and the network buffer as one total. Both
# connectors are parked in `none` first, so nothing moves between the three
# reads and the totals are of one instant.
measure() { # measure -> SOURCE SINK POOL TOTAL
    command_reply "createaddition mode $CX1 100 0 none" "$MODE_REPLY"
    command_reply "createaddition mode $CX2 100 0 none" "$MODE_REPLY"
    info "$AX" 100 0
    SOURCE=$energy
    [ "$kind" = accumulator ] || fail "the source at $AX is a $kind"
    info "$B" 100 0
    SINK=$energy
    info "$CX1" 100 0
    POOL=$((pool_ready + pool_next))
    NOW=$t
    TOTAL=$((SOURCE + SINK + POOL))
    command_reply "createaddition mode $CX1 100 0 pull" "$MODE_REPLY"
    command_reply "createaddition mode $CX2 100 0 push" "$MODE_REPLY"
}

seed_source() { # seed_source AMOUNT
    seed_at "$AX" 100 "$1"
}

link() { # link X1 X2 WIRE
    command_reply "createaddition link $1 100 0 $2 100 0 $3" "$LINK_REPLY"
    LAST_REPLY=$REPLY
    echo "  link $1-$2 ($3): $LAST_REPLY" >&2
}

unlink() { # unlink X1 X2
    command_reply "createaddition unlink $1 100 0 $2 100 0" "$WIRE_REPLY"
    LAST_REPLY=$REPLY
    echo "  unlink $1-$2: $LAST_REPLY" >&2
}

set_mode() { # set_mode X MODE
    command_reply "createaddition mode $1 100 0 $2" "$MODE_REPLY"
    LAST_REPLY=$REPLY
    echo "  mode $1 $2: $LAST_REPLY" >&2
}

# The control for EMBEDDED=1: the same binary with the embedded content made
# unavailable, where the ids must be unknown.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    arena
    ctl give createaddition:connector 1 >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "createaddition:connector")] | length == 0' >/dev/null \
        || fail "the control game has the connector item, so the content did not come from the embedded file"
    ctl command "execute in minecraft:the_end run setblock $CX1 100 0 createaddition:connector" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    local name
    name=$(block_name "$CX1" 100 0)
    [ "$name" != "createaddition:connector" ] || fail "the control game placed a connector, so the block id was known"
    echo "  the control's setblock left $(block_name "$CX1" 100 0) (the arena floor)" >&2
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no connector item, the block stayed $name"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle wrote its content out byte for byte.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/createaddition-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/createaddition.hmod" \
        || fail "the embedded content differs from content/createaddition.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/createaddition.hmod"
}

echo "== launch (The End arena, content + native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch "$WORLD"
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

ctl give createaddition:modular_accumulator 1 >/dev/null
ctl wait 4 >/dev/null
ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "createaddition:modular_accumulator")] | length >= 1' >/dev/null \
    || fail "the accumulator item did not register: $(ctl inventory | jq -c '{hotbar,main}')"

echo "== A: the four blocks, their buffers and the placement mode =="
place_accumulator "$AX"
place_accumulator "$B"
place_connector "$CX1" west
place_connector "$CX2" east
info "$CX1" 100 0
[ "$kind" = connector ] || fail "C1 has no block entity (info says $kind)"
[ "$capacity" = 1000 ] || fail "a small connector's window is $capacity, expected 1000"
[ "$mode" = pull ] || fail "C1 placed beside an accumulator took mode '$mode', expected pull"
[ "${large:-0}" = 0 ] || fail "C1 is a large connector"
# 1.7.2 declares a `Passive` mode its own wrench can never reach (the port
# drops it: `## Upstream defects fixed` in notes.md), so setting it must be
# refused and the mode must stay as it was.
command_reply "createaddition mode $CX1 100 0 passive" "$MODE_REFUSAL"
[ "$REPLY" = "The mode is none, pull or push." ] \
    || fail "the unreachable passive mode was not refused: $REPLY"
info "$CX1" 100 0
[ "$mode" = pull ] || fail "the refused mode still changed the connector to '$mode'"
info "$AX" 100 0
[ "$kind" = accumulator ] || fail "the source has no block entity"
[ "$capacity" = "$CAPACITY" ] || fail "the accumulator's capacity is $capacity, expected $CAPACITY"
[ "$energy" = 0 ] || fail "a fresh accumulator holds $energy FE"

echo "== B: 100,000 FE crosses the wire at 1,000 FE a tick =="
seed_source "$SEED"
set_mode "$CX2" push
link "$CX1" "$CX2" gold
[ "$LAST_REPLY" = "Nodes linked." ] || fail "the wire was not laid: $LAST_REPLY"
measure
BEFORE_SOURCE=$SOURCE
BEFORE_SINK=$SINK
BEFORE_POOL=$POOL
BEFORE_NOW=$NOW
BEFORE_TOTAL=$TOTAL
[ "$BEFORE_TOTAL" = "$SEED" ] || fail "the seeded $SEED FE totals $BEFORE_TOTAL across source, sink and network"
ctl wait "$TICKS"
measure
MOVED=$((BEFORE_SOURCE - SOURCE))
GAINED=$((SINK - BEFORE_SINK))
POOL_DELTA=$((POOL - BEFORE_POOL))
# `t=` on the two reads is the game clock: it also counts the ticks the four
# mode commands and the six reads take, so the window is $TICKS plus that
# overhead. The cap is asserted over $TICKS plus a generous allowance for those
# commands; at 1,000 FE a tick the flow must also be near the cap.
ELAPSED=$((NOW - BEFORE_NOW))
[ "$ELAPSED" -ge "$TICKS" ] || fail "only $ELAPSED ticks passed, expected at least $TICKS"
[ "$MOVED" -gt 0 ] || fail "no energy left the source over $ELAPSED ticks"
[ "$MOVED" -le $((RATE * (TICKS + 10))) ] \
    || fail "$MOVED FE moved over $ELAPSED ticks, more than the ${RATE}/tick a small connector allows"
[ "$MOVED" -ge $((RATE * (TICKS - 4))) ] \
    || fail "$MOVED FE moved over $ELAPSED ticks ($((MOVED / (ELAPSED > 0 ? ELAPSED : 1))) FE/tick), far below the ${RATE}/tick expected"
[ "$GAINED" = $((MOVED - POOL_DELTA)) ] \
    || fail "the source lost $MOVED FE, the network gained $POOL_DELTA and the sink gained $GAINED"
if [ "$SELFTEST" = 1 ]; then
    # The deliberately wrong expectation: the total MUST have changed. FE is
    # conserved, so this leg fails — which is the proof the scenario can fail.
    [ "$TOTAL" != "$BEFORE_TOTAL" ] || fail "SELFTEST: the conservation assertion did not trip"
fi
[ "$TOTAL" = "$BEFORE_TOTAL" ] \
    || fail "FE was not conserved across the wire: $BEFORE_TOTAL -> $TOTAL (source $BEFORE_SOURCE->$SOURCE, sink $BEFORE_SINK->$SINK)"
echo "  source $BEFORE_SOURCE->$SOURCE, sink $BEFORE_SINK->$SINK, network buffer $POOL, total conserved at $TOTAL"

echo "== C: CONTROL — the same two connectors with no wire move nothing =="
unlink "$CX1" "$CX2"
[ "$LAST_REPLY" = "Wire removed." ] || fail "the wire was not removed: $LAST_REPLY"
info "$CX1" 100 0
[ "$links" = 0 ] || fail "C1 still holds $links wire(s) after the unlink"
measure
CUT_SOURCE=$SOURCE
CUT_SINK=$SINK
CUT_TOTAL=$TOTAL
ctl wait "$TICKS"
measure
[ "$SOURCE" = "$CUT_SOURCE" ] || fail "the source moved $CUT_SOURCE->$SOURCE FE with no wire"
[ "$SINK" = "$CUT_SINK" ] || fail "the sink moved $CUT_SINK->$SINK FE with no wire"
[ "$TOTAL" = "$CUT_TOTAL" ] || fail "the total moved $CUT_TOTAL->$TOTAL with no wire"
echo "  no wire: source $SOURCE, sink $SINK, total $TOTAL, unchanged over $TICKS ticks"

echo "== D: a wire past a small connector's 16 blocks is refused =="
place_accumulator $((FAR_A - 1))
place_accumulator $((FAR_B + 1))
place_connector "$FAR_A" west
place_connector "$FAR_B" east
link "$FAR_A" "$FAR_B" copper
case "$LAST_REPLY" in
    *"too far apart"*) echo "  refused at 17 blocks, as the jar's 16-block limit says" ;;
    *) fail "a 17-block wire was not refused: $LAST_REPLY" ;;
esac
info "$FAR_A" 100 0
[ "$links" = 0 ] || fail "the refused wire was recorded anyway ($links link(s))"

echo "== E: copper cannot join two large connectors, gold can =="
place_accumulator $((BIG_A - 1))
place_accumulator $((BIG_B + 1))
place_connector "$BIG_A" west createaddition:large_connector
place_connector "$BIG_B" east createaddition:large_connector
info "$BIG_A" 100 0
[ "$capacity" = 5000 ] || fail "a large connector's window is $capacity, expected 5000"
link "$BIG_A" "$BIG_B" copper
case "$LAST_REPLY" in
    *"gold or electrum"*) echo "  copper refused between two large connectors" ;;
    *) fail "copper was not refused between two large connectors: $LAST_REPLY" ;;
esac
info "$BIG_A" 100 0
[ "$links" = 0 ] || fail "the refused copper wire was recorded anyway"
link "$BIG_A" "$BIG_B" gold
[ "$LAST_REPLY" = "Nodes linked." ] || fail "gold was refused between two large connectors: $LAST_REPLY"

echo "== F: an accumulator feeds the block above it (tickOutput UP) =="
# The block above is a connector held in `none`, so its 1,000 FE window only
# takes energy: nothing drains it, and its own pull is off. Two stacked
# accumulators would trade energy both ways — that pair is upstream's
# multiblock, a recorded gap.
STACK=$((X0 + 7))
place_accumulator "$STACK" 100
place_connector "$STACK" down createaddition:connector 101
command_reply "createaddition mode $STACK 101 0 none" "$MODE_REPLY"
# Empty both: a connector in `none` above an empty accumulator must stay empty,
# which is the control that nothing but the accumulator's UP output can fill it.
command_reply "createaddition energy $STACK 101 0 0" "$ENERGY_REPLY"
command_reply "createaddition energy $STACK 100 0 0" "$ENERGY_REPLY"
ctl wait 10
info "$STACK" 100 0
EMPTY_FE=$energy
info "$STACK" 101 0
EMPTY_WINDOW=$energy
[ "$EMPTY_FE" = 0 ] && [ "$EMPTY_WINDOW" = 0 ] \
    || fail "with both empty the pair reads $EMPTY_FE/$EMPTY_WINDOW, so something else fills the window"
seed_at "$STACK" 100 10000
ctl wait 12
info "$STACK" 100 0
FE=$energy
info "$STACK" 101 0
WINDOW=$energy
[ "$WINDOW" -gt 0 ] || fail "the block above took nothing from the accumulator below"
[ "$WINDOW" = 1000 ] \
    || fail "the block above holds $WINDOW FE, expected its 1,000 FE window filled by the accumulator"
[ $((FE + WINDOW)) = 10000 ] \
    || fail "the accumulator and the block above total $((FE + WINDOW)) FE, not the 10000 seeded"
echo "  accumulator 10000->$FE, the connector above 0->$WINDOW, total conserved at $((FE + WINDOW))"

echo "PASS: FE crossed the wire at 1,000/t and was conserved, an unlinked connector moved nothing, the 16-block limit held, copper was refused between two large connectors, and an accumulator fed the block above it"
