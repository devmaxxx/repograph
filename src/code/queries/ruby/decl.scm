; A module joins what it holds with `/` and a class with `.`. `class A::B` names its path, which
; reads `A/B` under the module around it. A class reopened in one file is one symbol, the first.

(module name: [
  (constant) @name
  (scope_resolution !scope name: (constant) @name)
  (scope_resolution scope: (constant) @name name: (constant) @name)
  (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name)
]) @definition.module

(class name: [
  (constant) @name
  (scope_resolution !scope name: (constant) @name)
  (scope_resolution scope: (constant) @name name: (constant) @name)
  (scope_resolution scope: (scope_resolution scope: (constant) @name name: (constant) @name) name: (constant) @name)
]) @definition.class

; A bare `private` or `protected` hides the methods after it in its body, until a bare `public`. A
; query cannot say "the last switch before", so the first pattern to match wins: a hide after a show
; hides, then a show after a hide shows. A body that hides, shows, hides and shows again is misread.
; A singleton class's body is read by the patterns below, so that a value's declares nothing.
((body_statement (identifier) @_show (identifier) @_hide (method name: (_) @name) @definition.method)
  (#eq? @_show "public")
  (#any-of? @_hide "private" "protected")
  (#set! export "never")
  (#set! outside "singleton_class"))

((body_statement (identifier) @_hide (identifier) @_show (method name: (_) @name) @definition.method)
  (#any-of? @_hide "private" "protected")
  (#eq? @_show "public")
  (#set! export "always")
  (#set! outside "singleton_class"))

((body_statement (identifier) @_hide (method name: (_) @name) @definition.method)
  (#any-of? @_hide "private" "protected")
  (#set! export "never")
  (#set! outside "singleton_class"))

((call !receiver method: (identifier) @_hide arguments: (argument_list . (method name: (_) @name) @definition.method .))
  (#any-of? @_hide "private" "protected")
  (#set! export "never")
  (#set! outside "singleton_class"))

((singleton_class value: (self) (body_statement (identifier) @_show (identifier) @_hide (method name: (_) @name) @definition.method))
  (#eq? @_show "public")
  (#any-of? @_hide "private" "protected")
  (#set! export "never"))

((singleton_class value: (self) (body_statement (identifier) @_hide (identifier) @_show (method name: (_) @name) @definition.method))
  (#any-of? @_hide "private" "protected")
  (#eq? @_show "public")
  (#set! export "always"))

((singleton_class value: (self) (body_statement (identifier) @_hide (method name: (_) @name) @definition.method))
  (#any-of? @_hide "private" "protected")
  (#set! export "never"))

((singleton_class value: (self) (body_statement (call !receiver method: (identifier) @_hide arguments: (argument_list . (method name: (_) @name) @definition.method .))))
  (#any-of? @_hide "private" "protected")
  (#set! export "never"))

; A method on a value's singleton class, `class << obj` as `def obj.f`, is that value's, which no
; name reaches; `def self.f` and a `class << self` body declare their class's, as `def f` does.
((method name: (_) @name) @definition.method
  (#set! outside "singleton_class"))

(singleton_class value: (self) (body_statement (method name: (_) @name) @definition.method))

(singleton_method object: (self) name: (_) @name) @definition.method

; `alias :new :old` names a symbol, whose colon is no part of the name, so it is left unread.
(alias name: [(identifier) (constant) (operator) (setter)] @name) @definition.method

(assignment left: (constant) @name) @definition.constant

; `require` and `require_relative` of one literal path, and `autoload :Name, "path"`. A path built
; at run time names no file the graph can find. `require` and `autoload` search the load path, never
; the requirer's directory, and `require_relative` that directory alone.
((call !receiver method: (identifier) @_require arguments: (argument_list . (string . (string_content) @import.path .) .)) @import
  (#eq? @_require "require")
  (#set! import "rooted"))

((call !receiver method: (identifier) @_require arguments: (argument_list . (string . (string_content) @import.path .) .)) @import
  (#eq? @_require "require_relative")
  (#set! import "relative"))

((call !receiver method: (identifier) @_autoload arguments: (argument_list . (simple_symbol) . (string . (string_content) @import.path .) .)) @import
  (#eq? @_autoload "autoload")
  (#set! import "rooted"))
