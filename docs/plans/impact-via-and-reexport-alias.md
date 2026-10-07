# fix/impact-via-and-reexport-alias

Based on main after #144 (object-literal members) landed.

## Tickets
- **#124: impact, changes: via shows an undeclared member instead of its symbol** (bug · open)
  - Description: `changes --base HEAD~1` printed `← …styleguide-gen.ts::KEYS.filter` as a `via`:
    an array method call folded as if it were a member of a literal. The #105 fold of an
    undeclared `X.m` into `X` reached the dependent rows, not `via`.
  - Acceptance criteria: `via` shows `X` for an undeclared member target, in `impact` and
    `changes`, text and JSON.
- **#109: impact: a renamed re-export resolves to its original** (bug · open)
  - Description: `theme/contrast.ts:27` re-exports `export { contrast as contrastRatio }`;
    `impact contrastRatio` answers "no node" and its 5 callers are lost. `impact::aliases`
    matches the original name, not the alias a re-export gives it.
  - Acceptance criteria: `impact <alias>` of a renamed re-export resolves to the original symbol
    and counts callers importing either name.

## Files to change
- `src/code/symbols.rs` — a renamed specifier is recorded as `orig as alias` in the `ReExports`
  context; a local `export { a as b }` writes a `ReExports` edge from the file to itself.
- `src/code/cases.rs` — extractor cases for both forms.
- `src/impact.rs` — re-export entries parsed into (original, exported); `aliases`, `exact` and
  `importers` follow a rename; the upstream walk seeds the root's undeclared `X.m` targets and
  names them `X` in `via`; `renamed` finds the declarations an alias stands for.
- `src/query.rs` — `candidates` falls back to a renamed re-export when no node matches.
- `docs/graph-model.md` — the rename and the `via` rule.

## Implementation steps
1. Extractor: `export_specifier` with an `alias` field → context entry `name as alias`, in both
   the `from` form and the local form (self-edge, only when some specifier renames).
2. `impact.rs`: one `entries(e)` parser; `aliases` walks (file, name) pairs, so a barrel that
   renames continues under the new name; `exact` maps an exported name back to its original;
   `importers` matches each file's imports against the name that file exports.
3. Upstream walk: seeds also take the `X.m` targets no node declares for each seed `X`, and a
   row reached through one shows `X` as `via`. Seeds only, not deeper frontiers — `--down` and
   `trace` land a folded target and do not walk on from it, so the four commands still agree.
4. `query::candidates`: no id, tail or label match ⇒ the declarations a renamed re-export names.

## Tests
- `src/code/cases.rs` — `a_renamed_re_export_records_both_names`, `a_local_renamed_export_re_exports_the_file_to_itself`.
- `src/impact.rs` — renamed re-export: `impact` of the original counts callers of both names,
  through a local rename and a renaming barrel; `canonical` of the alias is the original;
  importers list both files. Undeclared member: `impact KEYS` counts `pick` (`KEYS.filter`) with
  `via` = `KEYS`, text and JSON; up/down/trace agree on the pair.
- `src/changes.rs` — a hunk on `KEYS` lists `pick` with `via` `KEYS`, text and JSON.
- `src/query.rs` — `resolve_code("contrastRatio")` picks `contrast`.

## Decisions
- Base: first stacked on #144, rebased onto main once it landed.
- #124 re-checked against #144: on that branch no `via` can be an undeclared member any more —
  the literal-wide `undeclared` seeding that produced `KEYS.filter` was reverted (55f8883), and
  #144 declares literal methods instead. But the revert left a gap the issue's own premise
  names ("the dependency is real"): `impact --down pick` and `trace pick KEYS` reach `KEYS`
  through `KEYS.filter`, while `impact KEYS` and `changes` on `KEYS` report nothing. Considered
  (a) close #124 with a regression test only, (b) restore root-level undeclared seeding with
  `via` = the container. Chose (b), seeds only — it is the fold #105 applies downward, so the
  four commands agree, and it carries the #124 label fix (19fce92's shape) with it. Not the
  pre-revert literal-wide heuristic: nothing is seeded past depth 1's frontier.
- `via` for an undeclared member reached through a barrel is the barrel's container
  (`sym:index.ts::KEYS`), not the declaration: a barrel alias in `via` stays the name the caller
  wrote, as 19fce92 decided.
- Rename encoding: considered a new edge kind, a separate context field, and `orig as alias`
  entries in the existing comma list. Chose the last — `exports()` is the only reader of the
  context, `explain` prints it as written, and no store format changes.
- Local `export { a as b }`: a `ReExports` self-edge, so `aliases`/`exact` need no second path;
  `importers` already skips the declaring file.
- Bench floors not read: the fixture `~/bench/beauty-crm-502e8a6d` is not on disk.
