//! Ruby's shapes, each on inline files, so the reading of a declaration, a require and a call is
//! pinned by the assertion rather than by a corpus. Ruby is a spec row and its two queries, with no
//! hook.

use crate::code::prose::testing::{ids, lines, repo};
use crate::model::{EdgeKind, Extraction};

const GLOBS: &[&str] = &["**/*.rb", "**/*.rake", "**/*.gemspec"];

fn extract(files: &[(&str, &str)], rel: &str) -> Extraction {
    let (_dir, resolver) = repo(GLOBS, files);
    let text = files.iter().find(|(f, _)| *f == rel).map(|(_, t)| *t).unwrap();
    crate::code::front::read(&resolver, rel, text)
}

const DECLS: &str = r#"# frozen_string_literal: true

module Rack
  # The version.
  RELEASE = "3.2"

  # Serves a request.
  # Twice over.
  class Server < Base
    DEFAULT = 1

    def call(env)
      env
    end

    def self.start(app) = new(app)

    alias run call

    class << self
      def logger; end
    end

    protected

    def options; end

    public

    def port; end

    private

    def boot; end

    def GET; end

    private def secret; end

    class Handler; end
  end

  module Utils
    module_function

    def escape(s) = s
  end

  class Other::Deep; end

  class Server
    def call(env); end
    def later; end
  end
end
"#;

#[test]
fn modules_join_with_a_slash_classes_with_a_dot_and_a_reopened_class_is_one_symbol() {
    let ex = extract(&[("lib/rack/server.rb", DECLS)], "lib/rack/server.rb");
    assert_eq!(ids(&ex), [
        "file:lib/rack/server.rb",
        "sym:lib/rack/server.rb::Rack",
        "sym:lib/rack/server.rb::Rack/Other/Deep",
        "sym:lib/rack/server.rb::Rack/RELEASE",
        "sym:lib/rack/server.rb::Rack/Server",
        "sym:lib/rack/server.rb::Rack/Server.DEFAULT",
        "sym:lib/rack/server.rb::Rack/Server.GET",
        "sym:lib/rack/server.rb::Rack/Server.Handler",
        "sym:lib/rack/server.rb::Rack/Server.boot",
        "sym:lib/rack/server.rb::Rack/Server.call",
        "sym:lib/rack/server.rb::Rack/Server.later",
        "sym:lib/rack/server.rb::Rack/Server.logger",
        "sym:lib/rack/server.rb::Rack/Server.options",
        "sym:lib/rack/server.rb::Rack/Server.port",
        "sym:lib/rack/server.rb::Rack/Server.run",
        "sym:lib/rack/server.rb::Rack/Server.secret",
        "sym:lib/rack/server.rb::Rack/Server.start",
        "sym:lib/rack/server.rb::Rack/Utils",
        "sym:lib/rack/server.rb::Rack/Utils/escape",
    ], "`class Other::Deep` names its path, `class << self` declares its class's, and the reopened `Server` adds `later` to the one symbol");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:lib/rack/server.rb -> sym:lib/rack/server.rb::Rack [export]",
        "sym:lib/rack/server.rb::Rack -> sym:lib/rack/server.rb::Rack/Other/Deep [export]",
        "sym:lib/rack/server.rb::Rack -> sym:lib/rack/server.rb::Rack/RELEASE [export]",
        "sym:lib/rack/server.rb::Rack -> sym:lib/rack/server.rb::Rack/Server [export]",
        "sym:lib/rack/server.rb::Rack -> sym:lib/rack/server.rb::Rack/Utils [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.DEFAULT [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.GET []",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.Handler [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.boot []",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.call [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.later [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.logger [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.options []",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.port [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.run [export]",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.secret []",
        "sym:lib/rack/server.rb::Rack/Server -> sym:lib/rack/server.rb::Rack/Server.start [export]",
        "sym:lib/rack/server.rb::Rack/Utils -> sym:lib/rack/server.rb::Rack/Utils/escape [export]",
    ], "a method under `private` or `protected` is hidden until a bare `public`, and `private def` hides its own");
    let node = |id: &str| ex.nodes.iter().find(|n| n.id == id).unwrap();
    let server = node("sym:lib/rack/server.rb::Rack/Server");
    assert_eq!((server.line, server.end), (9, 41), "the first of a reopened class's definitions is the symbol");
    assert_eq!(server.body, "# Serves a request.\n# Twice over.\nclass Server < Base");
    assert_eq!(node("sym:lib/rack/server.rb::Rack/RELEASE").body, "# The version.\nRELEASE = \"3.2\"", "a module's first statement reads the comment above it");
    assert_eq!(node("sym:lib/rack/server.rb::Rack/Server.call").body, "def call(env)");
}

#[test]
fn a_require_names_a_file_under_lib_and_a_require_relative_one_beside_the_requirer() {
    let files = [
        ("lib/rack.rb", "module Rack\n  autoload :Builder, \"rack/builder\"\n  autoload :Lint, \"rack/#{name}\"\nend\n"),
        ("lib/rack/builder.rb", "require_relative 'utils'\nrequire 'rack/media'\nrequire 'set'\nrequire \"rack\"\nrequire_relative \"../../missing\"\n"),
        ("lib/rack/utils.rb", "module Rack; module Utils; end; end\n"),
        ("lib/rack/media.rb", ""),
        ("lib/rack/rack.rb", ""),
        ("test/spec_builder.rb", "require_relative '../lib/rack/builder'\nrequire_relative 'helper'\n"),
        ("test/helper.rb", ""),
        ("Rakefile.rake", "require_relative 'lib/rack'\n"),
        ("rack.gemspec", "require_relative \"lib/rack/media\"\n"),
    ];
    assert_eq!(lines(&extract(&files, "lib/rack.rb"), EdgeKind::Imports), [
        "file:lib/rack.rb -> file:lib/rack/builder.rb [*]",
    ], "an autoload of a literal path names its file, and one built at run time names nothing");
    assert_eq!(lines(&extract(&files, "lib/rack/builder.rb"), EdgeKind::Imports), [
        "file:lib/rack/builder.rb -> file:lib/rack.rb [*]",
        "file:lib/rack/builder.rb -> file:lib/rack/media.rb [*]",
        "file:lib/rack/builder.rb -> file:lib/rack/utils.rb [*]",
    ], "`require \"rack\"` names `lib/rack.rb`, not the `rack.rb` beside the requirer, a gem names nothing, and a path outside the repository nothing");
    assert_eq!(lines(&extract(&files, "test/spec_builder.rb"), EdgeKind::Imports), [
        "file:test/spec_builder.rb -> file:lib/rack/builder.rb [*]",
        "file:test/spec_builder.rb -> file:test/helper.rb [*]",
    ]);
    assert_eq!(lines(&extract(&files, "Rakefile.rake"), EdgeKind::Imports), ["file:Rakefile.rake -> file:lib/rack.rb [*]"]);
    assert_eq!(lines(&extract(&files, "rack.gemspec"), EdgeKind::Imports), ["file:rack.gemspec -> file:lib/rack/media.rb [*]"]);
}

#[test]
fn a_require_searches_lib_alone_and_a_require_relative_the_requirer_s_directory_alone() {
    let files = [
        (
            "lib/app/server.rb",
            "require \"logger\"\nrequire \"app/router.rb\"\nrequire_relative \"view.rb\"\nrequire_relative \"app\"\nmodule App\n  autoload :Logger, \"logger\"\nend\n",
        ),
        ("lib/app/logger.rb", ""),
        ("lib/app/router.rb", ""),
        ("lib/app/view.rb", ""),
        ("lib/app.rb", ""),
    ];
    assert_eq!(lines(&extract(&files, "lib/app/server.rb"), EdgeKind::Imports), [
        "file:lib/app/server.rb -> file:lib/app/router.rb [*]",
        "file:lib/app/server.rb -> file:lib/app/view.rb [*]",
    ], "the gem `logger` is no sibling `logger.rb`, a written `.rb` is not added twice, and `require_relative \"app\"` never falls back to `lib/app.rb`");
}

const REQUEST: &str = r#"module Rack
  class Request < Base
    include Helpers

    def initialize(env)
      @env = env
      parse(env)
      self.check
      Utils.escape(env)
      Rack::Utils.escape(env)
      Request.build
      headers = {}
      headers.merge!(size)
      size
      Missing.go
      Helpers::TOKEN
      ::Rack::Utils
    end

    def parse(env) = env

    def check; end

    def size = 1

    def self.build; end

    def headers; end

    module Helpers
      TOKEN = 1

      def host
        parse(1)
        helper
      end
    end
  end

  class Base; end
end

def helper; end
"#;

const UTILS: &str = "module Rack\n  module Utils\n    def self.escape(s) = s\n  end\nend\n";

#[test]
fn a_bare_call_reaches_its_class_and_the_file_and_a_self_call_its_owner() {
    let files = [("lib/rack/request.rb", REQUEST), ("lib/rack/utils.rb", UTILS)];
    assert_eq!(lines(&extract(&files, "lib/rack/request.rb"), EdgeKind::Calls), [
        "sym:lib/rack/request.rb::Rack/Request.Helpers/host -> sym:lib/rack/request.rb::Rack/Request.parse []",
        "sym:lib/rack/request.rb::Rack/Request.Helpers/host -> sym:lib/rack/request.rb::helper []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/request.rb::Rack/Request.build []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/request.rb::Rack/Request.check []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/request.rb::Rack/Request.parse []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/request.rb::Rack/Request.size []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/utils.rb::Rack/Utils/escape []",
    ], "a local hides the method it is named like, a nested module's method reaches the class around it, which mixes it in, and the top level's, `Utils.escape` is a path found in another file, and an unknown constant's call names nothing");
}

#[test]
fn a_constant_is_read_under_each_container_around_it_before_it_is_read_as_written() {
    let files = [
        ("lib/rack/request.rb", REQUEST),
        ("lib/rack/utils.rb", UTILS),
        ("lib/rack/base.rb", "module Rack\n  class Base; end\nend\n"),
        ("test/spec_request.rb", "describe Rack::Request do\n  it { Rack::Request::Helpers::TOKEN; Request }\nend\n"),
    ];
    let ex = extract(&files, "lib/rack/request.rb");
    // `Request.build` names the class the method sits in, which writes no edge, and `Missing` names
    // nothing anywhere.
    assert_eq!(lines(&ex, EdgeKind::References), [
        "sym:lib/rack/request.rb::Rack/Request -> sym:lib/rack/request.rb::Rack/Request.Helpers []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/request.rb::Rack/Request.Helpers/TOKEN []",
        "sym:lib/rack/request.rb::Rack/Request.initialize -> sym:lib/rack/utils.rb::Rack/Utils []",
    ]);
    assert_eq!(lines(&ex, EdgeKind::Extends), [
        "sym:lib/rack/request.rb::Rack/Request -> sym:lib/rack/request.rb::Rack/Base []",
    ], "a superclass is read outside its class, and the own file's `Base` shadows another file's");
    assert_eq!(lines(&extract(&files, "test/spec_request.rb"), EdgeKind::References), [
        "file:test/spec_request.rb -> sym:lib/rack/request.rb::Rack/Request []",
        "file:test/spec_request.rb -> sym:lib/rack/request.rb::Rack/Request.Helpers/TOKEN []",
    ], "a full path reaches a nested name in any file, and a bare `Request` outside `Rack` names nothing");
}

#[test]
fn a_reopened_module_is_reached_in_every_file_that_declares_it() {
    let files = [
        ("lib/a.rb", "module Shared; end\n"),
        ("lib/b.rb", "module Shared; end\n"),
        ("lib/c.rb", "class User\n  include Shared\nend\n"),
    ];
    assert_eq!(lines(&extract(&files, "lib/c.rb"), EdgeKind::References), [
        "sym:lib/c.rb::User -> sym:lib/a.rb::Shared []",
        "sym:lib/c.rb::User -> sym:lib/b.rb::Shared []",
    ]);
}

#[test]
fn a_block_parameter_a_rescued_error_and_a_loop_variable_hide_a_method_of_their_name() {
    let source = "class Job\n  def run\n    [1].each { |step| step }\n    begin\n    rescue => failure\n      failure\n    end\n    for item in [] do item end\n    step\n  end\n\n  def step; end\n  def failure; end\n  def item; end\nend\n";
    assert_eq!(lines(&extract(&[("job.rb", source)], "job.rb"), EdgeKind::Calls), [
        "sym:job.rb::Job.run -> sym:job.rb::Job.step []",
    ], "only the `step` outside the block is a call");
}

#[test]
fn a_hidden_method_stays_out_of_another_file_s_reach() {
    let files = [
        ("lib/a.rb", "module A\n  class << self\n    private\n    def hidden; end\n  end\n\n  def self.open; end\nend\n"),
        ("lib/b.rb", "A.hidden\nA.open\n"),
    ];
    assert_eq!(lines(&extract(&files, "lib/b.rb"), EdgeKind::Calls), ["file:lib/b.rb -> sym:lib/a.rb::A/open []"]);
}

#[test]
fn a_constant_passes_over_a_method_spelled_like_it() {
    let files = [(
        "lib/r.rb",
        "module R\n  POST = 'POST'\n  module Helpers\n    def POST; end\n    def post?; verb == POST; end\n    def params; self.POST; end\n  end\nend\n",
    )];
    let ex = extract(&files, "lib/r.rb");
    assert_eq!(lines(&ex, EdgeKind::References), ["sym:lib/r.rb::R/Helpers/post? -> sym:lib/r.rb::R/POST []"]);
    assert_eq!(lines(&ex, EdgeKind::Calls), ["sym:lib/r.rb::R/Helpers/params -> sym:lib/r.rb::R/Helpers/POST []"]);
}

#[test]
fn an_id_in_a_comment_or_string_is_cited_by_what_holds_it() {
    let ex = extract(&[("a.rb", "# ADR-004 explains the wait\ndef wait\n  puts \"FR-APP-54\"\nend\n")], "a.rb");
    assert_eq!(lines(&ex, EdgeKind::References), ["file:a.rb -> ADR-004 [comment]", "sym:a.rb::wait -> FR-APP-54 [string]"]);
}

#[test]
fn a_ruby_file_is_read_by_its_extension_and_a_rake_file_too() {
    use crate::code::lang::{Family, Lang};
    for rel in ["lib/rack.rb", "bin/tool.rbw", "rack.gemspec", "tasks/release.rake"] {
        assert_eq!(Lang::of(rel), Some(Lang::Ruby), "{rel}");
    }
    assert_eq!(Lang::Ruby.family(), Family::Ruby);
}

#[test]
fn a_name_in_an_endless_body_a_when_clause_or_a_loop_header_is_read_as_one_in_a_statement_is() {
    let source = "class A\n  def a = b\n  def self.s = t\n  def c; case 1 when d then 1 end; end\n  def e; for i in f do end; end\n  def g\n    case h\n    in 1 then 1\n    end\n  end\n  def b; end\n  def t; end\n  def d; end\n  def f; end\n  def h; end\nend\n";
    assert_eq!(lines(&extract(&[("a.rb", source)], "a.rb"), EdgeKind::Calls), [
        "sym:a.rb::A.a -> sym:a.rb::A.b []",
        "sym:a.rb::A.c -> sym:a.rb::A.d []",
        "sym:a.rb::A.e -> sym:a.rb::A.f []",
        "sym:a.rb::A.g -> sym:a.rb::A.h []",
        "sym:a.rb::A.s -> sym:a.rb::A.t []",
    ]);
}

#[test]
fn a_value_s_singleton_class_declares_nothing_and_self_s_hides_as_a_class_body_does() {
    let source = "class << obj\n  private\n  def hid; end\n  def pub; end\nend\n\nclass A\n  class << other\n    def plain; end\n    private def own; end\n  end\n\n  class << self\n    private\n    def boot; end\n    public\n    def open; end\n    private def secret; end\n  end\nend\n";
    let ex = extract(&[("a.rb", source)], "a.rb");
    assert_eq!(lines(&ex, EdgeKind::Declares), [
        "file:a.rb -> sym:a.rb::A [export]",
        "sym:a.rb::A -> sym:a.rb::A.boot []",
        "sym:a.rb::A -> sym:a.rb::A.open [export]",
        "sym:a.rb::A -> sym:a.rb::A.secret []",
    ], "`private` in `class << obj` declares nothing, as the same body without it does");
}
