//! PHP's shapes, each on inline files, so the reading of a declaration, a `use` and a call is pinned
//! by the assertion rather than by a corpus. PHP is a spec row and its two queries, with no hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.php"];

const COMPOSER: &str = r#"{
    "name": "acme/shop",
    "autoload": { "psr-4": { "Shop\\": "src/", "Shop\\Legacy\\": ["lib/legacy", "lib/compat/"] } },
    "autoload-dev": { "psr-4": { "Shop\\Tests\\": "tests" } }
}"#;

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const CART: &str = r#"<?php

declare(strict_types=1);

namespace Shop;

/**
 * A cart of lines.
 */
#[Entity]
final class Cart extends Base implements \Countable, Priced
{
    use Discounts;

    public const MAX = 10;
    private const SECRET = 'k', SALT = 's';

    /** The lines. */
    private array $lines = [];
    public static ?Cart $shared = null, $last;
    var $legacy;

    public function __construct(private Clock $clock, public readonly int $size, int $plain)
    {
        $helper = new class {
            public function inner(): void {}
        };
        $total = function () { return 1; };
    }

    /**
     * Adds a line.
     */
    public function add(
        Line $line,
        int $count = 1,
    ): self {
        return $this;
    }

    protected function check(): bool { return true; }

    function count(): int { return 0; }
}

interface Priced extends Base, Other { public function price(): int; }

trait Discounts { private function discount() {} }

enum Status: string implements Priced
{
    case Open = 'open';
    const DEFAULT = self::Open;
    public function price(): int { return 0; }
}

function cart_total(Cart $cart): int { return 0; }

if (!function_exists('Shop\polyfill')) {
    function polyfill() {}
}
"#;

#[test]
fn the_file_declares_its_types_with_their_members_and_its_functions() {
    let ex = extract(&[("composer.json", COMPOSER), ("src/Cart.php", CART)], "src/Cart.php");
    assert_eq!(ids(&ex), [
        "file:src/Cart.php",
        "sym:src/Cart.php::Cart",
        "sym:src/Cart.php::Cart.MAX",
        "sym:src/Cart.php::Cart.SALT",
        "sym:src/Cart.php::Cart.SECRET",
        "sym:src/Cart.php::Cart.__construct",
        "sym:src/Cart.php::Cart.add",
        "sym:src/Cart.php::Cart.check",
        "sym:src/Cart.php::Cart.clock",
        "sym:src/Cart.php::Cart.count",
        "sym:src/Cart.php::Cart.last",
        "sym:src/Cart.php::Cart.legacy",
        "sym:src/Cart.php::Cart.lines",
        "sym:src/Cart.php::Cart.shared",
        "sym:src/Cart.php::Cart.size",
        "sym:src/Cart.php::Discounts",
        "sym:src/Cart.php::Discounts.discount",
        "sym:src/Cart.php::Priced",
        "sym:src/Cart.php::Priced.price",
        "sym:src/Cart.php::Status",
        "sym:src/Cart.php::Status.DEFAULT",
        "sym:src/Cart.php::Status.Open",
        "sym:src/Cart.php::Status.price",
        "sym:src/Cart.php::cart_total",
        "sym:src/Cart.php::polyfill",
    ], "an anonymous class's method and a closure are no declaration, a promoted parameter is a property, and a plain one is not");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:src/Cart.php -> sym:src/Cart.php::Cart [export]",
        "file:src/Cart.php -> sym:src/Cart.php::Discounts [export]",
        "file:src/Cart.php -> sym:src/Cart.php::Priced [export]",
        "file:src/Cart.php -> sym:src/Cart.php::Status [export]",
        "file:src/Cart.php -> sym:src/Cart.php::cart_total [export]",
        "file:src/Cart.php -> sym:src/Cart.php::polyfill [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.MAX [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.SALT []",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.SECRET []",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.__construct [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.add [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.check []",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.clock []",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.count [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.last [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.legacy [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.lines []",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.shared [export]",
        "sym:src/Cart.php::Cart -> sym:src/Cart.php::Cart.size [export]",
        "sym:src/Cart.php::Discounts -> sym:src/Cart.php::Discounts.discount []",
        "sym:src/Cart.php::Priced -> sym:src/Cart.php::Priced.price [export]",
        "sym:src/Cart.php::Status -> sym:src/Cart.php::Status.DEFAULT [export]",
        "sym:src/Cart.php::Status -> sym:src/Cart.php::Status.Open [export]",
        "sym:src/Cart.php::Status -> sym:src/Cart.php::Status.price [export]",
    ], "`private` and `protected` keep a member to its class, and a member written with neither is public");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let cart = node("sym:src/Cart.php::Cart");
    assert_eq!((cart.line, cart.end), (10, 44), "a class starts at its attributes");
    assert_eq!(cart.body, "/**\n * A cart of lines.\n */\n#[Entity] final class Cart extends Base implements \\Countable, Priced");
    assert_eq!(
        node("sym:src/Cart.php::Cart.add").body,
        "/**\n     * Adds a line.\n     */\npublic function add( Line $line, int $count = 1, ): self",
        "a signature over several lines reads as one"
    );
    assert_eq!(node("sym:src/Cart.php::Cart.lines").body, "/** The lines. */\nprivate array $lines = [];");
    assert_eq!(node("sym:src/Cart.php::Cart.SECRET").line, 16);
}

const ROUTE: &str = r#"<?php

namespace Shop\Routing;

use Shop\Cart;
use Shop\Http\Kernel as K;
use Shop\Missing;
use Vendor\Lib\Client;
use Shop\Legacy\Old;
use Shop\Legacy\Compat;
use function Shop\cart_total;
use const Shop\LIMIT;
use Shop\Http\{Request, Response};

class Route extends Group implements Routable
{
    use Steps;

    public const NAME = 'route';

    public function run(Cart $cart, Helper $other): ?Group
    {
        $this->check();
        $this?->check();
        self::make();
        static::make();
        parent::run($cart, $other);
        Cart::create();
        K::boot();
        Group::make();
        Cart::hidden();
        $cart->add();
        $c = new Cart();
        if ($c instanceof Group) {}
        try {} catch (Failure $e) {}
        tidy(Cart::class, Cart::MAX, self::NAME);
        \Shop\Cart::create();
        return null;
    }

    private function check(): void {}

    public static function make(): static { return new static(); }
}
"#;

const SIBLINGS: [(&str, &str); 11] = [
    ("composer.json", COMPOSER),
    ("src/Cart.php", "<?php\nnamespace Shop;\n\nclass Cart {\n    public const MAX = 3;\n    public static function create() {}\n    private static function hidden() {}\n    public function add() {}\n}\n"),
    ("src/Http/Kernel.php", "<?php\nnamespace Shop\\Http;\n\nclass Kernel { public static function boot() {} }\n"),
    ("src/Routing/Group.php", "<?php\nnamespace Shop\\Routing;\n\nclass Group { public static function make() {} public function run() {} }\n"),
    ("src/Routing/Routable.php", "<?php\nnamespace Shop\\Routing;\n\ninterface Routable {}\n"),
    ("src/Routing/Steps.php", "<?php\nnamespace Shop\\Routing;\n\ntrait Steps {}\n\nclass Failure extends \\Exception {}\n\nfunction tidy(...$a) {}\n"),
    ("src/Routing/Helper.php", "<?php\nnamespace Elsewhere;\n\nclass Helper {}\n"),
    ("lib/legacy/Old.php", "<?php\nnamespace Shop\\Legacy;\n\nclass Old {}\n"),
    ("lib/compat/Compat.php", "<?php\nnamespace Shop\\Legacy;\n\nclass Compat {}\n"),
    ("src/Legacy/Old.php", "<?php\nnamespace Shop\\Legacy;\n\nclass Old {}\n"),
    ("src/Routing/Route.php", ROUTE),
];

#[test]
fn a_use_names_the_file_composer_maps_its_namespace_to_by_the_longest_prefix() {
    let ex = extract(&SIBLINGS, "src/Routing/Route.php");
    assert_eq!(lines(&ex, EdgeKind::Imports), [
        "file:src/Routing/Route.php -> file:lib/compat/Compat.php [*]",
        "file:src/Routing/Route.php -> file:lib/legacy/Old.php [*]",
        "file:src/Routing/Route.php -> file:src/Cart.php [*]",
        "file:src/Routing/Route.php -> file:src/Http/Kernel.php [*]",
    ], "`Shop\\Legacy\\` outranks `Shop\\`, and each of its directories is tried; a class with no file, a vendor's class, a function, a constant and a group name nothing");
}

#[test]
fn a_class_resolves_through_its_use_its_alias_and_its_namespace() {
    let ex = extract(&SIBLINGS, "src/Routing/Route.php");
    assert_eq!(lines(&ex, EdgeKind::Calls), [
        "sym:src/Routing/Route.php::Route.run -> sym:src/Cart.php::Cart.create []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Http/Kernel.php::Kernel.boot []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Group.php::Group.make []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Route.php::Route.check []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Route.php::Route.make []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Steps.php::tidy []",
    ], "`parent::run()`, a private method of another class, `$cart->add()` and a fully qualified receiver stay missing");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:src/Routing/Route.php::Route -> sym:src/Routing/Steps.php::Steps [impl]",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Cart.php::Cart [type]",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Cart.php::Cart.MAX []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Group.php::Group [type]",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Route.php::Route.NAME []",
        "sym:src/Routing/Route.php::Route.run -> sym:src/Routing/Steps.php::Failure [type]",
    ], "a class of another namespace in the same directory is not seen without a `use`");
    assert_eq!(lines(&ex, EdgeKind::Extends), ["sym:src/Routing/Route.php::Route -> sym:src/Routing/Group.php::Group []"]);
    assert_eq!(lines(&ex, EdgeKind::Implements), ["sym:src/Routing/Route.php::Route -> sym:src/Routing/Routable.php::Routable []"]);
}

#[test]
fn a_test_namespace_reads_its_own_prefix_and_reaches_the_class_it_uses() {
    let mut files = SIBLINGS.to_vec();
    files.push(("tests/Routing/RouteTest.php", "<?php\nnamespace Shop\\Tests\\Routing;\n\nuse Shop\\Routing\\Route;\n\nclass RouteTest {\n    public function testRun() { Route::make(); $r = new Route(); }\n}\n"));
    let ex = extract(&files, "tests/Routing/RouteTest.php");
    assert_eq!(lines(&ex, EdgeKind::Imports), ["file:tests/Routing/RouteTest.php -> file:src/Routing/Route.php [*]"]);
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:tests/Routing/RouteTest.php::RouteTest.testRun -> sym:src/Routing/Route.php::Route.make []"]);
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:tests/Routing/RouteTest.php::RouteTest.testRun -> sym:src/Routing/Route.php::Route [type]"]);
}

#[test]
fn the_nearest_composer_json_decides_even_when_it_maps_nothing() {
    let files = [
        ("composer.json", COMPOSER),
        ("src/Cart.php", "<?php\nnamespace Shop;\n\nclass Cart {}\n"),
        ("tools/composer.json", r#"{ "name": "acme/tools" }"#),
        ("tools/run.php", "<?php\nnamespace Tools;\n\nuse Shop\\Cart;\n"),
        ("bin/run.php", "<?php\nuse Shop\\Cart;\n"),
    ];
    assert_eq!(lines(&extract(&files, "tools/run.php"), EdgeKind::Imports), Vec::<String>::new());
    assert_eq!(lines(&extract(&files, "bin/run.php"), EdgeKind::Imports), ["file:bin/run.php -> file:src/Cart.php [*]"]);
}

#[test]
fn an_empty_prefix_maps_every_namespace_after_the_longer_ones() {
    let files = [
        ("composer.json", r#"{ "autoload": { "psr-4": { "": "src/", "Lib\\": "lib/" } } }"#),
        ("src/App.php", "<?php\nuse Lib\\Tool;\nuse Web\\Page;\n"),
        ("src/Web/Page.php", "<?php\nnamespace Web;\n\nclass Page {}\n"),
        ("lib/Tool.php", "<?php\nnamespace Lib;\n\nclass Tool {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "src/App.php"), EdgeKind::Imports), [
        "file:src/App.php -> file:lib/Tool.php [*]",
        "file:src/App.php -> file:src/Web/Page.php [*]",
    ]);
}

#[test]
fn two_classes_of_one_name_in_reach_resolve_to_neither() {
    let files = [
        ("composer.json", COMPOSER),
        ("src/A/Thing.php", "<?php\nnamespace Shop\\A;\n\nclass Thing {}\n"),
        ("src/B/Thing.php", "<?php\nnamespace Shop\\B;\n\nclass Thing {}\n"),
        ("src/C/User.php", "<?php\nnamespace Shop\\C;\n\nuse Shop\\A\\Thing;\nuse Shop\\B\\Thing as Other;\n\nfunction f(Thing $t, Other $o) {}\n"),
        ("src/C/Thing.php", "<?php\nnamespace Shop\\C;\n\nclass Thing {}\n"),
    ];
    assert_eq!(lines(&extract(&files, "src/C/User.php"), EdgeKind::References), Vec::<String>::new(),
        "the use and the namespace both offer `Thing`, and a type named by its alias stays missing");
}

#[test]
fn text_before_the_php_tag_is_read_as_markup() {
    let ex = extract(&[("views/page.php", "<html><body>\n<?php\nfunction render() { helper(); }\nfunction helper() {}\n?>\n</body></html>\n")], "views/page.php");
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:views/page.php::render -> sym:views/page.php::helper []"]);
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.php", "<?php\n// ADR-004 explains the wait\n# ADR-005 too\nfunction wait() { echo \"FR-APP-54\"; }\n")], "a.php");
    assert_eq!(lines(&ex, EdgeKind::References), [
        "file:a.php -> ADR-004 [comment]",
        "file:a.php -> ADR-005 [comment]",
        "sym:a.php::wait -> FR-APP-54 [string]",
    ]);
}

#[test]
fn a_php_file_is_read_by_its_extension() {
    use crate::code::lang::{Family, Lang};
    assert_eq!(Lang::of("src/App.php"), Some(Lang::Php));
    assert_eq!(Lang::Php.family(), Family::Php);
}

fn widened(stale: &[&str], removed: &[&str], g: &crate::model::Graph, all: &[&str]) -> Vec<String> {
    let owned = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let mut w = crate::code::engine::widen(&owned(stale), &owned(removed), g, &owned(all));
    w.sort();
    w
}

#[test]
fn a_changed_class_widens_to_its_directory_and_its_importers() {
    let mut g = crate::model::Graph::default();
    g.edges.insert(crate::model::Edge {
        source: "file:tests/CartTest.php".into(),
        target: "file:src/Cart.php".into(),
        kind: EdgeKind::Imports,
        context: "*".into(),
        file: "tests/CartTest.php".into(),
    });
    let all = ["src/Cart.php", "src/Line.php", "src/Http/Kernel.php", "tests/CartTest.php", "src/notes.md"];
    assert_eq!(widened(&["src/Cart.php"], &[], &g, &all), ["src/Line.php", "tests/CartTest.php"]);
}
