# embedder.sh — which weights every row of this kit is read under. Sourced by the scripts that run
# a writer or a reader; sourcing defines the two calls below and runs nothing, so
# `test_embedder.py` reads both refusals with no store under them.
#
# Declared in the environment rather than in a `repograph.toml` beside the store, for two reasons
# the file could not give. `REPOGRAPH_EMBED_MODEL` outranks what a store records, so a store
# restored under a changed built-in default is still read and embedded with the weights the reading
# names. And the corpus worktree stays clean: `repograph.toml` is untracked and unignored there and
# `changes` reads untracked paths, so a file of ours would have every `changes` row mapping two
# changed paths instead of the one the row touched.
EMBEDDER=intfloat/multilingual-e5-small

# Refused, never defaulted, in both directions. Unset, the binary falls back to whatever its
# built-in default is this month — that was `e5-large` between `35357c1` and 2026-09-07 under
# ADR-002 — and the reading could not say which weights answered. Naming another model, the row
# would be read under weights this machine does not hold cached, and quietly replacing the value
# would hide from an operator that they had asked for something else.
embedder_declared() {
  case ${REPOGRAPH_EMBED_MODEL-} in
    "$EMBEDDER")
      # Exported and not only checked: a row is read under what the binary's own process sees, and
      # a declaration that arrived as a shell variable would stop at the script.
      export REPOGRAPH_EMBED_MODEL
      return 0 ;;
    "") echo "refusing: REPOGRAPH_EMBED_MODEL is not set — export REPOGRAPH_EMBED_MODEL=$EMBEDDER before running this kit; unset, the binary takes whatever its built-in default is today and no reading can say which weights it was taken under" >&2 ;;
    *) echo "refusing: REPOGRAPH_EMBED_MODEL names $REPOGRAPH_EMBED_MODEL — every row of this kit is read under $EMBEDDER, and only its weights are cached on this machine" >&2 ;;
  esac
  return 2
}

# Beside the rows in the run's own summary, the way the quiet line is: a median with no embedder
# named next to it does not say which weights answered.
embedder_line() {
  echo "embedder: REPOGRAPH_EMBED_MODEL=$REPOGRAPH_EMBED_MODEL"
}
