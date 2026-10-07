# feat/explain-edge-location

## Tickets
- **#136: explain: two edges from one id look like a duplicate without their file** (GitHub issue · open · milestone 0.6.0)
  - Description: two documents can declare one id. In beauty-crm,
    `docs/adr/ADR-008-domain-twice-conformance.md` and `docs/adr/ADR-008-appendix-port-journal.md`
    both resolve to `ADR-008` and both mention `FR-CAL-40`, so `explain FR-CAL-40` prints
    `References ← ADR-008  [prose]` twice, and `--json` repeats
    `{kind: References, dir: in, other: ADR-008, context: prose}`. The rows read as a duplicated edge.
  - Acceptance criteria: a neighbour row carries where the reference is written (`path:line`) in
    the text form and in `--json`, so two edges from one id are told apart. Display only: answers
    (`ask`, `impact`, `trace`, counts) do not change.
  - Comments: none.
  - Open questions: none; the location source is a decision below.

## Files to change
- `src/query.rs` — `explain` appends the edge's location to each row; `explain_json` gives each
  edge `file` and `line`; one helper computes the location for both; unit tests.
- `tests/json_surface.rs` — pin the new edge fields on the CLI's `--json`.
- `README.md` — the `explain --json` paragraph names `file` and `line`.

## Implementation steps
1. Add `edge_location(graph, edge) -> (&str, Option<u32>)`: the edge's own `file`, and a line
   read off the one endpoint written in that file — for `Declares` the declared target, for
   every other kind the source (the referrer), never the other end; it counts only when its primary
   `file` is the edge's file and it is not a `File` node (a file node's line is always 1, which
   would claim a precision the graph does not have).
2. `explain`: append `  path:line` (or `  path` when no line is known) to every neighbour row.
3. `explain_json`: each edge gains `file` and `line` (`null` when unknown).
4. Update the unit tests whose rows ended at the neighbour id.

## Tests
- `src/query.rs` — `explain_tells_two_declarers_of_one_id_apart_by_file` — two docs declaring
  `ADR-008`, both citing `FR-CAL-40`: two rows, each naming its own file, the primary one with
  the declaration line; same in `--json`.
- `src/query.rs` — `explain_places_a_call_at_its_caller` — a `Calls` row carries the caller's
  `file:line`; a `Declares` row carries the declared node's line, not the file node's.
- `tests/json_surface.rs` — `explain --json` edges carry `file` and `line`.

## Decisions
- Where the line comes from: considered (a) a new `line` field on `Edge`, (b) reading the edge's
  file from disk at display time and finding the id, (c) the line of the endpoint declared in the
  edge's file; chose (c). (a) touches 176 `ex.edge` call sites, changes the postcard mirror shape
  (a `RGM3` magic) and — because `Edge` is a `BTreeSet` key — turns one edge per document into
  one per mention, which moves edge counts and `impact` totals: not display only. (b) needs the
  repo root inside `query`, costs a read per neighbour file, and on `--stale` can name a line the
  stored graph never saw. (c) is free and exact about the file, which is what tells the two rows
  apart; the line is the enclosing declaration (the requirement block, the calling function), and
  is left out rather than guessed when no endpoint is declared in that file. No store or mirror
  version bump: the store format is unchanged.
- Every row carries its location, not only rows that would otherwise repeat: the issue asks for
  it per row, and a reader cannot know a row is unique without seeing the others.
- Branch `feat/explain-edge-location` was free locally and on origin; cut from `origin/main`.
