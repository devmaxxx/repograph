"""What counts as a reference, on the lines that were miscounted."""

import unittest

from truth import strip_comments


class StripComments(unittest.TestCase):
    def test_a_name_inside_a_single_quoted_token_is_not_a_reference(self):
        # apps/api/test/loggingModule.spec.ts:11 — a logger's name, not a dependency.
        src = "const OUTBOX_LOGGER_TOKEN = 'PinoLogger:OutboxPublisher';\n"
        self.assertNotIn("OutboxPublisher", strip_comments(src))

    def test_a_name_inside_an_error_message_is_not_a_reference(self):
        # apps/api/src/shared/db/database.service.ts:34 — prose about the interceptor.
        src = 'throw new Error("a route with :businessId behind TenantContextInterceptor");\n'
        self.assertNotIn("TenantContextInterceptor", strip_comments(src))

    def test_a_template_literal_is_blanked_too(self):
        self.assertNotIn("AuthService", strip_comments("log(`${x} AuthService`);\n"))

    def test_code_outside_strings_and_comments_survives_with_its_line_count(self):
        src = (
            "import { AuthService } from './auth.service.js'; // AuthService\n"
            "/* AuthService */\n"
            "new AuthService('x');\n"
        )
        out = strip_comments(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertEqual(out.count("AuthService"), 2)

    def test_a_url_in_a_string_is_not_a_line_comment(self):
        self.assertIn("fetch(", strip_comments("fetch('http://x/y');\n"))

    def test_a_multi_line_template_literal_keeps_the_lines_after_it_numbered(self):
        # A template literal spanning several lines must not collapse them into
        # the line where it opens: everything after it still counts from its own line.
        src = "const q = `select *\nfrom t\nwhere x`;\nnew AuthService();\n"
        out = strip_comments(src)
        self.assertEqual(out.count("\n"), src.count("\n"))
        self.assertIn("AuthService", out)

    def test_known_limitation_a_quote_in_a_regex_literal_desyncs_the_rest_of_the_line(self):
        # A tokeniser would know `/'/` is a regex, not a string; this scan does not.
        # It opens a string on that quote and closes on the real string's opening quote,
        # leaving the real string's body — and any symbol in it — un-blanked. Documented
        # in strip_comments' docstring as a known loss; this pins today's actual output
        # so a future fix is noticed here rather than silently changing behaviour.
        src = "const r = /'/; const s = 'AuthService';\n"
        self.assertEqual(strip_comments(src), "const r = /''AuthService';\n")


if __name__ == "__main__":
    unittest.main()
