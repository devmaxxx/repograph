# Languages

Which grammar reads a file is decided by its extension alone. A file the globs reach that no
grammar reads is indexed as a file node and nothing more, and `build`, and any refresh that re-reads
such a file, names those extensions once on stderr — `code: no grammar reads 2 .cs — indexed as files
only` — so a glob that reaches past the
grammars says so instead of producing a graph silently thinner than the tree.

Every language is read the same way: two tree-sitter queries and its row of data, with hooks for
what the data cannot say ([graph-model.md](graph-model.md) says how). Every language in the table
below, and every one after it but Razor, is read by default. The first `build`, `update` or refreshing
`ask` after upgrading from 0.5.x re-reads the whole tree once, because this release reads files by
a new grammar generation; the next reads only what changed. `ZEROGRAPH_CODE_GLOBS` withdraws a
language for one run without writing config into the tree. A `zerograph.toml` that names `code_globs`
keeps exactly the list it names, so a 0.5.x config that wrote the old default out reads no new
language until the line is removed or widened.

| language | extensions | reads | not read |
|---|---|---|---|
| TypeScript, JavaScript | `.ts` `.tsx` `.mts` `.cts` `.js` `.jsx` `.mjs` `.cjs` | exports and declarations with class members; imports through relative paths, `tsconfig.json` paths and `package.json` exports; calls; requirement ids cited in comments and strings; decorators | `.mts` and `.cts` are read when globbed by name; the default `skip` keeps bundles out (`**/*.min.js`, `**/.yarn/**`, `**/.pnp.*`) |
| C# | `.cs` | types — classes, records, structs, interfaces, enums, delegates — with their members, primary-constructor parameters and nested types; `namespace` blocks and file-scoped namespaces; `using`, `using static`, aliases, `global::`, and a project's `global using`s; base types; partial types across files; calls through fields, properties, parameters, locals, `this`, `base`, static types and extension methods, and a typed value's calls up its base chain; static member access `X.Member` and generic arguments on expression calls (`AddScoped<IFoo, Foo>()`) as type uses; `typeof`, `default` and casts; attributes; requirement ids in comments and strings; generated files like any other | a call on `this`, a bare name or a static type to a member it inherits, and a call through an inherited field or property; a type found only through a using, which links as `X.Member` only when invoked as `X.M()`, since any other shape could be a package's namespace; `dynamic`, reflection and registrations in a DI container; a `.csproj` edit, which widens nothing until the next `build` (accepted in 0.6.0) |
| Kotlin | `.kt` | classes, interfaces, `fun interface`, enums, data, sealed and annotation classes, objects; top-level `fun`, `val`, `var`, `typealias`, and extensions named without their receiver; members, constructor `val`s and companion members as `Class.member`; nested declarations; `Extends`, `Calls` through parameters, locals, typed properties and objects, `Imports`, cited ids, and `DecoratedBy` to an annotation declared in the repository; names resolve through one index shared with Java, and a call to an `expect` binds it and the caller's platform `actual`, or every `actual` from common code | `.kts` Gradle scripts, indexed as files only; calls on a value whose type is inferred from anything but a constructor, through another file's property types, inside a lambda passed to another file's `T.() -> R` function, to a companion's members from a nested type, a lowercase bare call under a supertype the repository does not declare, and `init` blocks; `private` and `protected` names are not exported, `internal` ones are |
| Java | `.java` | classes, interfaces, enums, records and annotation types; methods, fields and constructors (`Type.Type`) as `Type.member`; nested types; `extends` and `implements` as `Extends`; `Calls` through parameters, locals, typed fields, `this.x`, `Type.m()` and `new T()` to the type it creates; a field's initializer walked from the field, and a static or instance initializer block from its class; `Imports` for `import a.b.C` and `import static a.b.C.m`, with `a.b.*` resolved at each use; cited ids; `DecoratedBy` to an annotation type declared in the repository; Kotlin and Java resolve each other | record components and enum constants as symbols; `super.` calls, call chains, calls through another file's fields, calls in an enum constant's arguments or body, and a lowercase bare call under a supertype the repository does not declare; a nested class in another file whose supertype is a member type it inherits, named like a top-level type of the package, may be read as extending the top-level one; anything not `public` is not exported, except an interface's members |
| SQL (PostgreSQL) | `.sql` | Several kinds of object are symbols, their schema joined as `app/clients`: tables, views, materialized views, functions, procedures, types, schemas and sequences. Columns, triggers, policies and indexes are members of their table (`app/clients.status`). Every migration that creates or alters an object declares it. `ALTER TABLE`, a foreign key and a view's `FROM` are references; a trigger calls its function. A table renamed by `RENAME TO` or `SET SCHEMA` is declared under its new name by the `ALTER`, which references the old one. An unquoted name folds to lower case. | PL/pgSQL bodies, though the ids they cite still reach `ask`; `DROP`, `GRANT` and `COMMENT ON`; domains, extensions and aggregates; edges from TypeScript query builders |
| GraphQL | `.gql` `.graphql` | Operations are `query/Name`, `mutation/Name` and `subscription/Name`, and fragments are `fragment/Name`. Types, inputs, enums, interfaces, unions, scalars and directives are symbols, and fields are `Type.field`. A spread calls its fragment wherever the repository declares it. `on T` and variable, field and argument types are references; `implements` extends; a directive decorates. | selections checked against the schema; enum values and arguments as symbols; edges from TypeScript |
| Rust | `.rs` | `fn`, `struct`, `enum`, `union`, `trait`, `type`, `const`, `static` and `macro_rules!`, with trait and `impl` members as `Type.member` and any `pub` as an export; the module tree from paths and every `Cargo.toml` — `crate::`, `self::`, `super::`, a workspace crate's name, `[lib] path`; `use` trees with braces, globs and `as`, `pub use` as a re-export, and a path through a column-0 `pub use a::b::C;` followed one hop to the declaring file; calls through paths, bound names, `self`, and fields whose struct declares their type, through `&`, `Box`, `Arc`, `Rc`, `dyn` and a type parameter's bound; `macro_rules!` invocations; an `impl` of another file's type, tied to that type; attributes and derives that resolve in the repository; requirement ids in comments and strings | `#[path]` modules; calls written inside a macro's arguments; a method on a receiver that is neither `self` nor a typed field; field-typed calls when the struct's `impl` is in another file; a re-export written with braces or a glob, or a chain of re-exports past the first hop; external crates |
| Python | `.py` | `def`, `class` and plain-name assignments in the module and in class bodies, nested classes as `Class.Inner` through two levels; exports by the leading underscore; imports resolved from the importing file's directory, each `pyproject.toml`, `setup.py` or `setup.cfg` directory above it — and its `src/` when the package is not beside the manifest — and the repository root, relative imports by package; a name a package's `__init__.py` re-exports with `from .x import …`, followed one hop to the declaring module; calls through imported and top-level names, modules, classes, `self`, and attributes typed by an annotation or by `self.x = T(…)` in `__init__`; base classes that resolve, as `Extends`; decorators that resolve in the repository; requirement ids in comments, docstrings and strings | declarations under `if __name__ == "__main__":` or any other top-level `if` or `try`; members of a class nested three deep; `__all__`; a local variable's type; a `global` or `nonlocal` name, which still reads as the function's local; `super()` calls; the names a wildcard import brings in; imports inside dotted directories (`.github/` and the like), which are extracted but which the resolver's walk skips; a `package-dir` a manifest names other than `src/` |
| Shell | `.sh` `.bash` | functions, all exported; `source` and `.` of a written-out path or one built from the script's own directory (`$(dirname "$0")`, `${BASH_SOURCE%/*}`, `$(cd … && pwd)`, or a variable assigned one); calls to a function of the script or of a script it sources; a script run by path or through `bash` or `sh`, recorded and not walked; requirement ids in comments and strings | a path through any other variable, `$HOME` or a glob; `eval`; aliases |
| Bicep | `.bicep` | `param`, `var`, `resource` (a child as `parent.child`), `module`, `type` and `func` by symbolic name, outputs as `output/<name>`; references between declarations of a file; a `module` path in the repository, reaching what that file declares; requirement ids in comments and strings | registry and template-spec modules (`br:`, `ts:`); `import … from`; `.bicepparam` |
| Terraform | `.tf` | Terraform addresses with `/` for `.` (`aws_instance/web`, `data/aws_ami/ubuntu`, `var/region`, `local/name`, `module/dns`, `output/ip`, `provider/hcloud`), each reference resolved within its module's directory; a local `module` source, reaching what that directory declares; requirement ids in comments and strings | `.tfvars`; registry modules; provider-defined functions |
| HCL | `.hcl` | other HCL by block type and label, with bake `inherits` and `targets`; requirement ids in comments and strings | `.terraform.lock.hcl`, which is a file and declares nothing; `${VAR}` in a bake file; a list inside a function call, such as `concat([…])` |
| Dart | `.dart` | classes, mixins, extensions, enums, typedefs, top-level functions and variables; members, fields and named constructors as `Type.member`; `extends`, `with` and `implements` as `Extends`; `Imports` through `package:` URIs, resolved by each `pubspec.yaml`'s `name:`, and relative URIs; `export` as `ReExports`; `part` and `part of` read as one library; `Calls` through the library's own names, imported names, typed fields and implicit `this` | `dart:` and external package imports; an unnamed extension's members; operators; cited requirement ids; a name with a leading `_` is not exported |
| Swift | `.swift` | classes, structs, enums, protocols, functions and top-level `let` constants; members as `Type.member`, with an extension's members named on the extended type; inheritance as `Extends` when a globbed Swift file declares the name | imports, calls and cited ids; properties, top-level `var`, `init`, enum cases, `typealias` and `actor`; `private` and `fileprivate` names are not exported |
| Vue | `.vue` | each `<script>` block, `setup` or not, through the TypeScript reading (`lang="tsx"` through TSX); the component as `<FileStem>`, exported and spanning its file, so a template edit changes it; decorators as `DecoratedBy` to a declaration the file declares or imports | the template, styles and custom blocks; a component used only as a template tag is not a reference |

Razor (`.razor`, `.cshtml`) is read only where `code_globs` or `ZEROGRAPH_CODE_GLOBS` names it: its
`@code` blocks are read as C#, but its readings did not pass `impact` and `trace` in 0.6.0.

Lua (`.lua`, and a script whose shebang names `lua` or `lua5.x`) is read by default.

- **Read:** `function f()`, `function M.f()` and `function M:m()` (as `M.f` and `M.m`), and
  `M.f = function`, at the file's level or in a `do` block or `if` branch there. A `local function`
  is the file's own and is not exported. `require("a.b")` and `require "a.b"` import `a/b.lua` or
  `a/b/init.lua` under the repository root, `src/` or `lua/`. A bare call reaches the file's own
  function, or the one global function a required file declares. `m.f()` and `m:f()` through a
  `require`'s alias reach the member `f` of the table the required file returns (`return M` or
  `return setmetatable(M, …)`), or its top-level function `f` when it returns no table; a file that
  ends in `return require("a")` hands on `a`'s members, and its import is a re-export. `M.f()` on the
  file's own table reaches the file's `M.f`, and so does `self:f()` inside `function M:g()`,
  `function M.g(self)` or `M.g = function(self)`, until a `local self` rebinds it. Requirement ids
  are read in comments and strings.
- **Not read:** a function defined inside another function's body, or two blocks deep;
  `M.f = function` inside a branch; a method called on a value (`obj:m()`) or one `self` inherits;
  `self` in a function with no `self` of its own (`function M.f()`); the members of a table a module
  returns as a constructor (`return { f = f }`); a `require` of a computed name, of a module outside
  the repository, or inside a larger expression (`require("a").b`, `require("a") or {}`), whose local
  is then a plain value; `luajit` shebangs.

Go (`.go`) is read by default.

- **Read:** functions; methods as `T.m` on their receiver's type, pointer or generic; structs,
  interfaces and other named types, with a struct's named fields and an interface's methods as
  `T.member`; package-level `const` and `var`. A name that starts with an upper-case letter is
  exported. A package is its directory: a bare name reaches the declaration in any file of it, and
  every one of build-tagged variants (`x_linux.go`, `x_windows.go`) unless the calling file declares
  the name itself. An import path under the module that the nearest `go.mod` names reaches the
  package directory's files, less `_test.go`. `pkg.F()` and `pkg.T` through an import's name, or the
  package's own name when the import has none, reach the exported member. `s.m()` on a method's
  receiver reaches the receiver type's method in any file of the package. Type uses are references:
  parameters, results, fields, composite literals, conversions, assertions, generic arguments and
  `var _ I = (*T)(nil)`. Requirement ids are read in comments and strings.
- **Not read:** a method called on any other value (`x.m()`), since its type is not inferred; a
  method promoted from an embedded field; dot imports; the standard library, other modules, and a
  `go.mod` edit, which widens nothing until the next `build`; a one-argument generic call
  (`f[T](x)`), which the grammar reads as a conversion, so it is a type use and not a call; the type in `node := &node{}`, which reads as the new local.

C (`.c`) is read by default. Its headers are read as C++, below.

- **Read:** function definitions, wherever a prototype stands, so the header's prototype and the
  source's definition are one symbol, the definition; function-like `#define`s as functions;
  structs, unions and enums with their members as `T.member`, named by the typedef when one names
  them, so `typedef struct reply { … } reply_t;` is `reply_t`; the members of an unnamed nested
  struct or union are the holder's; enumerators; typedefs of anything else. A `static` name stays in
  its file; any other links across the build. A bare call reaches the file's own definition, else
  the one non-`static` definition anywhere in the repository. `#include "x.h"` reaches `x.h` beside
  the including file, else at the root, under `include/` or under `src/`. Definitions are read in
  every `#ifdef` branch and inside `extern "C" { … }`; two branches defining one name are one
  symbol. Requirement ids are read in comments and strings.
- **Not read:** a prototype or an `extern` declaration; a call through a function pointer or a
  member (`p->fn()`, `ops.fn()`); a name a macro builds; a `static` function a header defines and
  its includers call (`static inline` helpers), since only the file that defines it reaches it;
  `#include <…>`, including one of the repository's own headers; a type or member a function body
  declares, and a type written in a variable's declaration (`static struct { … } config;`); a
  definition the grammar cannot parse, as when a branch of `#if` repeats a function's header.

C++ (`.cc`, `.cpp`, `.cxx`, `.c++`, `.ino`, and the headers `.h`, `.hh`, `.hpp`, `.hxx`, `.h++`) is read
by default. `.cu` is not read. A `.h` is read as C++ whether a C or a C++ file includes it: the C++
grammar parses C's headers no worse, and reads a class. C and C++ link as one build.

- **Read:** everything C reads; classes, structs and unions as `T.member`, with their methods,
  constructors, destructors, operators, pure virtual methods and data members, nested types as
  `Outer.Inner`, and members under `#if` inside the class; a method defined outside its class
  (`Status DB::Get(…) { … }`) as the member of the class nearest its name, a template around that
  class being a class too (`List<K>::Iterator::Next` is `List.Iterator.Next`); `using` aliases.
  Namespaces qualify nothing, so `leveldb::DBImpl` is `DBImpl`. A friend defined in its class, and
  a function of an unnamed namespace, are functions of the file; the latter stays in it, as a
  `static` one does. Export macros and thread-safety annotations (`class LIB_EXPORT Status`,
  `int n_ GUARDED_BY(mu_);`) and a macro standing alone as a member (`Q_OBJECT`) are read past. A
  bare call inside a method reaches its class's member first, wherever the class is declared, then
  what a C call reaches; `ns::f()` reaches `f` as a bare call does.
- **Not read:** a prototype, a deleted or defaulted method, or a method declared in the class and
  never defined; a call on a value (`o.f()`, `p->f()`) or a class's static member (`A::f()`),
  since types are not inferred; `new T(…)`, a base or member initialiser, and `T x(f());`, which the
  grammar reads as a declaration; overloads, which are one symbol; template instantiations; a
  function a test macro defines (`TEST(…) { … }` is a function named `TEST`); a class a function
  body declares.

Ruby (`.rb`, `.rbw`, `.gemspec` and `.rake`) is read by default.

- **Read:** modules and classes, with modules joining what they hold with `/` and classes with `.`,
  so `module Rack; class Request; module Helpers` holds `Rack/Request.Helpers/host`; `class A::B`
  under its path; `def` and `def self.` methods, and the methods of a `class << self` body, as the
  class's; `alias`; constants. A class or module reopened in one file is one symbol, the first
  definition; reopened in several files, each file declares it and a reference reaches every one.
  A method after a bare `private` or `protected`, or written as `private def`, stays out of other
  files' reach. A constant is read under each class and module around it, innermost first, then as
  written, and reaches the file that declares that full name anywhere in the repository, since
  autoloading needs no `require`. `A::B::C` paths, and `A::B.f` and `self.f` calls, resolve the same
  way. A bare call reaches a method of a class or module around it in its own file, or a top-level
  one there. `require_relative` of one literal path reaches `path.rb` beside the requiring file
  only; `require` and `autoload :X, "path"` reach it under `lib/` only, as Ruby's load path does,
  so `require "logger"` names no sibling `logger.rb`. A path already ending in `.rb` is taken as
  written. The superclass is an Extends edge.
  Requirement ids are read in comments and strings.
- **Not read:** a method called on a variable or any other value (`req.host`), since its type is
  not inferred; a method a mixin (`include`) brings, unless the module sits inside the class that
  mixes it in; `include` and `extend` themselves, which are read as plain references to the module;
  `def obj.f` and the body of `class << obj` on anything but `self`, after a `private` or not; `alias :new :old` with symbols;
  `define_method`, `attr_*` and other metaprogramming; a call written `stat(…)` whose name a local
  of the same name shadows (`stat = stat(path)`); a nested class's base when it is the class around
  it; gems and the standard library.

PHP (`.php`) is read by default.

- **Read:** classes, interfaces, traits and enums, with their methods, properties (a constructor's
  promoted parameters among them), constants and enum cases as `Class.member`; functions. A member
  marked `private` or `protected` is hidden; any other, and every type and function, is exported.
  A `use A\B\C;` or `use A\B\C as D;` reaches the file the nearest `composer.json`'s PSR-4 map
  (`autoload` and `autoload-dev`) gives the class, by the longest prefix. A class name reaches the
  own file's class, else the one class of that name among the files the `use`s reach and the same
  directory's files under the same namespace; two such classes resolve to neither. `$this->m()`, `self::m()` and `static::m()` reach the own class's method; `X::m()`,
  `X::NAME` and `X::class` reach `X`'s public member or `X` itself. Type uses are references:
  parameter, return and property types, `new X`, `instanceof X`, `catch (X $e)`, and trait uses.
  Text before `<?php` is read as markup. Requirement ids are read in comments and strings.
- **Not read:** a method called on any other value (`$x->m()`), since its type is not inferred; a
  method or constant inherited from a parent or a trait; `parent::m()`; a fully qualified name
  (`\A\B\C`); a type named only by its `use … as` alias; group uses (`use A\{B, C}`), `use function`
  and `use const`; `require` and `include`; classes of other packages under `vendor/`; a
  `composer.json` edit, which widens nothing until the next `build`.

Scala (`.scala`, and `.sc` scripts) is read by default. An sbt build
definition (`.sbt`) is never read.

- **Read:** classes, traits, objects and enums, with their `def`s as methods, `val`s and `var`s as
  properties, `type`s and enum cases as `Type.member`, nested types under their path (`read.lines`,
  `Path./`). A class and its companion object are one symbol, as are a method's overloads. A
  package object's and the top level's definitions are the package's functions and values. A
  member marked `private` or `protected` is hidden; one with a qualifier (`private[os]`) stays
  visible. Names are keyed by their package's full path across directories: a file's
  `package a.b` clauses, nested or dotted, put its names in `a.b`. A name is read through the
  file's imports (`import a.b.C`, `import a.b.{C, D}`, `import a.b._`), then under its own packages,
  innermost first, then as written, and reaches each file that declares it, so a name two source
  variants (`src-2`, `src-3`) declare reaches both. An import reaches the files that declare what it
  names, and a wildcard every file of the package. A bare call or `this.m()` reaches a member of the
  type around it first; `os.read(p)`, `Path.apply(…)` and `os.remove.all(p)` reach what that path
  declares. `extends` and every `with` are Extends edges; type uses (parameters, results, type
  arguments, `new X`, typed patterns, annotations) are references. Requirement ids are read in
  comments and strings.
- **Not read:** a call on a value (`p.relativeTo(q)`), since its type is not inferred; a member
  inherited through `this`; implicit conversions and parameters, `given`s and extension methods; a
  field read with no call (`os.pwd`); a renamed or hidden import selector (`D => E`, `D => _`);
  an auxiliary constructor (`def this(…)`), which `new` names as the class; definitions inside a
  function body, a block, a `val`'s initialiser or an anonymous class; libraries outside the
  repository.

Elixir (`.ex` and `.exs`) is read by default.

- **Read:** `defmodule`, `defprotocol` and `defimpl`, under Elixir's own dotted names, a nested
  module joining its parent's with `.`; `def`, `defmacro`, `defguard` and `defdelegate`, with
  `defp`, `defmacrop` and `defguardp` hidden from other modules. Every clause and arity of one name
  is one symbol, and the `@doc` and `@spec` above it are its doc. A module is global by its full
  name: a name is read under each module around it, innermost first, then as written. `alias X`
  and `alias X, as: Y` make the alias stand for `X` after it, in the module or function that holds
  the directive; `import X` makes `X`'s functions callable bare there, after the own module's.
  A bare call `f()`, a pipe's `|> f` and a capture `&f/1` reach the own module's function first;
  `Mod.f()` and `__MODULE__.f()` reach `Mod`'s or the own module's public function. A module named
  anywhere (a directive, a struct, a call's module, `@behaviour`) is a reference. Requirement ids
  are read in comments and strings.
- **Not read:** functions a `use` brings in, and anything else a macro writes; a bare call inside a
  `quote`, which runs in the caller's module; a call in a module attribute; `alias X.{A, B}`;
  `import`'s `only:` and `except:`, so an import opens every function; `defstruct` and
  `defexception`, whose struct is its module; a call on a variable (`mod.f()`); Kernel and other
  packages' modules. `alias`, `import`, `require` and `use` name modules, never files, so they write
  no Imports edge.

OCaml (`.ml`, read by the implementation grammar, and `.mli`, read by the interface grammar) is read
by default.

- **Read:** top-level `let` bindings (each of a `let rec … and …`), `external`, `val` in an
  interface, exceptions, classes and methods; types, with their constructors and record fields as
  `t.Field`; modules and module types, a module joining what it holds with `/`, so
  `module Export = struct let f` is `Export/f`. Every top-level definition is exported, an `.mli`
  beside it notwithstanding. A module is its file: `Foo` is `foo.ml` (else `foo.mli`) beside the
  user, else the one file of that name in the repository. Every module path a file names is an
  import of that file, as ocamldep reads it, unless the file binds the name itself first
  (`module Re = Core`); `include M` and `module A = M` hand `M` on. A name is read in its own file
  under each module around it, then in the module a `let open M in` or `M.(…)` around it opens,
  then in the one module the file's `open`s hold it in, then through the module its first segment
  names, with what that module includes and aliases. A type's name reaches a type, any other name
  a value. Requirement ids are read in comments and strings.
- **Not read:** a `let` inside a function, a `let … in`, and pattern names, which are locals;
  `.mli` exports, so a value an interface hides is still exported; functors, functor applications
  (`module S = Set.Make (…)`) and first-class modules; dune's library wrapping, so `Lib.M` from
  outside the library names nothing, and a standard-library module a repository file spells
  (`Str`, `Domain`) reads that file; the standard library and other packages; a file added later
  that makes a module's name ambiguous, which widens nothing until the next `build`.
