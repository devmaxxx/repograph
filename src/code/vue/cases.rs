//! Vue single-file components on synthetic sources: which bytes are script, and what the
//! TypeScript walk writes from them.

use crate::code::imports::Resolver;
use crate::code::CodeExtractor;
use crate::config::Config;
use crate::model::{EdgeKind, Extraction, Extractor};

const CHECKOUT: &str = "<template>
  <div class=\"checkout\">
    <p>Pay with <script> in a sentence</p>
    <template v-if=\"ready\"><Cart /></template>
  </div>
</template>

<!-- <script>not a block</script> -->
<script lang=\"ts\">
  import { Component, Vue } from 'vue-property-decorator';
  import Cart from '@/components/Cart.vue';
  import { total } from '@/pricing';
  import { Audited } from '@/audit';

  @Component({ components: { Cart } })
  @Audited
  export default class Checkout extends Vue {
    amount = 0;
    pay(): number { return total(this.amount); }
  }
</script>

<style scoped>
.checkout { color: red; }
</style>
";

fn portal() -> Vec<(&'static str, &'static str)> {
    vec![
        ("tsconfig.json", "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"src/*\"] } } }"),
        ("src/components/Cart.vue", "<template>\n  <ul class=\"cart\"></ul>\n</template>\n"),
        ("src/components/Checkout.vue", CHECKOUT),
        ("src/pricing.ts", "export function total(n: number): number { return n; }\n"),
        ("src/audit.ts", "export function Audited(target: unknown) { return target; }\n"),
        ("src/routes.ts", "import Checkout from '@/components/Checkout.vue';\n\nexport const routes = [{ component: Checkout }];\n"),
    ]
}

fn extract_in(files: &[(&str, &str)], globs: &[&str], rel: &str) -> Extraction {
    let dir = tempfile::tempdir().unwrap();
    for (p, c) in files {
        let full = dir.path().join(p);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(full, c).unwrap();
    }
    let cfg = Config { code_globs: globs.iter().map(|g| g.to_string()).collect(), ..Config::default() };
    let src = files.iter().find(|(p, _)| *p == rel).unwrap().1;
    CodeExtractor::new(Resolver::new(dir.path(), &cfg).unwrap()).extract(rel, src)
}

const ALL: &[&str] = &["**/*.ts", "**/*.tsx", "**/*.vue"];

fn ids(ex: &Extraction) -> Vec<&str> {
    ex.nodes.iter().map(|n| n.id.as_str()).collect()
}

fn edges(ex: &Extraction, kind: EdgeKind) -> Vec<(&str, &str, &str)> {
    ex.edges.iter().filter(|e| e.kind == kind).map(|e| (e.source.as_str(), e.target.as_str(), e.context.as_str())).collect()
}

#[test]
fn only_top_level_script_elements_are_blocks() {
    let blocks = super::script_blocks(CHECKOUT);
    assert_eq!(blocks.len(), 1, "{blocks:?}");
    assert!(CHECKOUT[blocks[0].content.clone()].trim_start().starts_with("import { Component, Vue }"));
    let two = "<script lang=\"ts\">export const a = 1;</script>\n<script setup lang='tsx'>const b = <i />;</script>\n";
    let blocks = super::script_blocks(two);
    assert_eq!(blocks.iter().map(|b| (b.setup, b.tsx)).collect::<Vec<_>>(), vec![(false, false), (true, true)]);
    assert!(super::script_blocks("<script lang=tsx>\n</script>\n")[0].tsx, "an unquoted value counts");
}

#[test]
fn the_blanked_copy_keeps_every_row_and_column() {
    let (blank, lang) = super::blanked(CHECKOUT).unwrap();
    assert_eq!(lang, crate::code::lang::Lang::TypeScript);
    assert_eq!(blank.len(), CHECKOUT.len());
    let (orig, copy): (Vec<&str>, Vec<&str>) = (CHECKOUT.lines().collect(), blank.lines().collect());
    assert_eq!(orig.len(), copy.len());
    assert_eq!(orig[16], copy[16], "the class line is kept where it is");
    assert!(copy[2].trim().is_empty(), "template text is blanked");
    assert!(copy[23].trim().is_empty(), "style is blanked");
}

#[test]
fn the_component_is_an_exported_symbol_spanning_the_file_and_its_class_members_are_its_members() {
    let ex = extract_in(&portal(), ALL, "src/components/Checkout.vue");
    let c = ex.nodes.iter().find(|n| n.id == "sym:src/components/Checkout.vue::Checkout").unwrap();
    assert_eq!((c.line, c.end), (1, 25));
    assert!(edges(&ex, EdgeKind::Declares).contains(&("file:src/components/Checkout.vue", "sym:src/components/Checkout.vue::Checkout", "export")));
    for id in ["sym:src/components/Checkout.vue::Checkout.pay", "sym:src/components/Checkout.vue::Checkout.amount"] {
        assert!(ids(&ex).contains(&id), "{id} missing from {:?}", ids(&ex));
    }
}

#[test]
fn a_component_with_no_script_is_a_file_and_a_symbol() {
    let ex = extract_in(&portal(), ALL, "src/components/Cart.vue");
    assert!(ids(&ex).contains(&"file:src/components/Cart.vue"));
    assert!(ids(&ex).contains(&"sym:src/components/Cart.vue::Cart"));
}

#[test]
fn a_typescript_import_of_a_vue_file_is_an_edge_only_when_vue_is_globbed() {
    let ex = extract_in(&portal(), ALL, "src/routes.ts");
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:src/routes.ts", "file:src/components/Checkout.vue", "Checkout")), "{:?}", ex.edges);
    let ex = extract_in(&portal(), &["**/*.ts", "**/*.tsx"], "src/routes.ts");
    assert!(!ex.edges.iter().any(|e| e.target.ends_with(".vue")), "{:?}", ex.edges);
}

#[test]
fn the_script_imports_and_calls_as_typescript_does() {
    let ex = extract_in(&portal(), ALL, "src/components/Checkout.vue");
    assert!(edges(&ex, EdgeKind::Imports).contains(&("file:src/components/Checkout.vue", "file:src/components/Cart.vue", "Cart")), "{:?}", ex.edges);
    assert!(edges(&ex, EdgeKind::Calls).contains(&("sym:src/components/Checkout.vue::Checkout.pay", "sym:src/pricing.ts::total", "")), "{:?}", ex.edges);
}

#[test]
fn a_decorator_points_at_its_declaration_or_at_nothing() {
    let ex = extract_in(&portal(), ALL, "src/components/Checkout.vue");
    let decorated = edges(&ex, EdgeKind::DecoratedBy);
    assert!(decorated.contains(&("sym:src/components/Checkout.vue::Checkout", "sym:src/audit.ts::Audited", "")), "{decorated:?}");
    assert!(!decorated.iter().any(|(_, t, _)| t.contains("Component")), "a decorator from a package names nothing: {decorated:?}");
    assert!(!ids(&ex).iter().any(|i| i.starts_with("deco:")), "{:?}", ids(&ex));
}

#[test]
fn a_call_in_an_options_api_or_setup_script_is_reachable_from_the_component() {
    let files = [
        ("src/pricing.ts", "export function total(n: number): number { return n; }\n"),
        ("src/Pay.vue", "<script lang=\"ts\">\nimport { defineComponent } from 'vue';\nimport { total } from './pricing';\nexport default defineComponent({\n  methods: { pay(): number { return total(1); } },\n});\n</script>\n"),
        ("src/Quick.vue", "<script setup lang=\"ts\">\nimport { total } from './pricing';\nconst sum = total(2);\n</script>\n"),
    ];
    for (rel, comp) in [("src/Pay.vue", "sym:src/Pay.vue::Pay"), ("src/Quick.vue", "sym:src/Quick.vue::Quick")] {
        let ex = extract_in(&files, ALL, rel);
        // What L11 walks: a call, or a reference to a symbol.
        let walked = |e: &&crate::model::Edge| e.kind == EdgeKind::Calls || (e.kind == EdgeKind::References && e.target.starts_with("sym:"));
        let mut reached = vec![comp.to_string()];
        let mut i = 0;
        while i < reached.len() {
            let from = reached[i].clone();
            for e in ex.edges.iter().filter(walked).filter(|e| e.source == from) {
                if !reached.contains(&e.target) {
                    reached.push(e.target.clone());
                }
            }
            i += 1;
        }
        assert!(reached.iter().any(|r| r == "sym:src/pricing.ts::total"), "{rel}: the call belongs to the component: {:?}", ex.edges);
        assert!(!ex.edges.iter().any(|e| e.kind == EdgeKind::Calls && e.source == format!("file:{rel}")), "{rel}: no call is the file's: {:?}", ex.edges);
    }
}

#[test]
fn a_component_added_while_watching_is_imported_on_the_next_poll() {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    std::fs::write(r.join("repograph.toml"), "doc_globs = []\ncode_globs = [\"**/*.ts\", \"**/*.vue\"]\n").unwrap();
    std::fs::write(r.join("main.ts"), "import Cart from './Cart.vue';\nexport const app = Cart;\n").unwrap();
    let cfg = crate::config::Config::load(r).unwrap();
    crate::run_update(r, &cfg, true).unwrap();
    let mut w = crate::Watcher::open(r, &cfg).unwrap();
    std::fs::write(r.join("Cart.vue"), "<template>\n  <ul></ul>\n</template>\n").unwrap();
    // main.ts is touched too, so the only thing under test is what the resolver knows.
    std::fs::write(r.join("main.ts"), "import Cart from './Cart.vue';\nexport const app = Cart;\n\n").unwrap();
    assert!(matches!(w.poll(1).unwrap(), crate::Polled::Refreshed(_)));
    assert!(
        w.graph.edges.iter().any(|e| e.kind == EdgeKind::Imports && e.source == "file:main.ts" && e.target == "file:Cart.vue"),
        "{:?}", w.graph.edges
    );
}

#[test]
fn a_tsx_script_is_read_by_the_tsx_grammar() {
    let ex = extract_in(&[("Table.vue", "<script lang=\"tsx\">\nexport function Row() { return <td>1</td>; }\n</script>\n")], ALL, "Table.vue");
    assert!(ids(&ex).contains(&"sym:Table.vue::Row"), "{:?}", ids(&ex));
}

/// Not a test: blanks every `.vue` file under `REPOGRAPH_CENSUS_ROOTS` and parses its scripts, as §0 did.
#[test]
#[ignore]
fn vue_census() {
    let roots = std::env::var("REPOGRAPH_CENSUS_ROOTS").expect("REPOGRAPH_CENSUS_ROOTS");
    let (mut files, mut scripts, mut bad) = (0, 0, 0);
    for root in roots.split(':') {
        // As `walk.rs` since 0.5.3 (#64): dotted paths read, `.git` pruned by name.
        for dent in ignore::WalkBuilder::new(root).hidden(false).filter_entry(|e| e.file_name() != ".git").git_ignore(true).build().flatten() {
            if dent.path().extension().and_then(|e| e.to_str()) != Some("vue") {
                continue;
            }
            files += 1;
            let src = std::fs::read_to_string(dent.path()).unwrap();
            scripts += super::script_blocks(&src).len();
            let Some((blank, lang)) = super::blanked(&src) else { continue };
            let tree = lang.parse(blank.as_bytes()).unwrap();
            let mut stack = vec![tree.root_node()];
            let mut broken = false;
            while let Some(n) = stack.pop() {
                broken |= n.is_error() || n.is_missing();
                let mut c = n.walk();
                stack.extend(n.children(&mut c));
            }
            bad += usize::from(broken);
        }
    }
    println!("Vue files {files} scripts {scripts} bad {bad}");
}
