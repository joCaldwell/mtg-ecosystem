"""Exercise documentation growth and link failures in disposable repositories."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location("check_docs", Path(__file__).with_name("check-docs.py"))
docs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(docs)


class DocumentationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        subprocess.run(["git", "init", "-q", self.root], check=True)

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def test_inventory_counts_untracked_and_all_markdown_extensions_but_not_ignored_outputs(self):
        self.write("README.md", "one")
        self.write("guide.MARKDOWN", "two")
        self.write("component.mdx", "three")
        self.write("removed.md", "removed")
        subprocess.run(["git", "-C", self.root, "add", "removed.md"], check=True)
        (self.root / "removed.md").unlink()
        self.write(".gitignore", "generated/\nnode_modules/\n")
        self.write("generated/report.md", "ignored evidence")
        self.write("node_modules/README.md", "ignored dependency")
        errors, count, words, _, _ = docs.inspect_docs(self.root)
        self.assertEqual((errors, count, words), ([], 3, 3))

    def test_budgets_accept_the_boundary_and_reject_file_and_total_growth(self):
        with patch.multiple(docs, MAX_FILES=2, MAX_WORDS=6, MAX_FILE_WORDS=3):
            self.write("one.md", "one two three")
            self.write("two.md", "four five six")
            self.assertEqual(docs.inspect_docs(self.root)[0], [])
            self.write("three.md", "seven eight nine ten")
            errors = docs.inspect_docs(self.root)[0]
            self.assertTrue(any("file budget" in error for error in errors))
            self.assertTrue(any("word budget;" in error for error in errors))
            self.assertTrue(any("three.md:" in error and "document budget" in error for error in errors))

    def test_fenced_examples_still_consume_the_word_budget(self):
        self.write("example.md", "```text\none two three four\n```\n")
        with patch.multiple(docs, MAX_WORDS=5, MAX_FILE_WORDS=5):
            errors, _, words, _, _ = docs.inspect_docs(self.root)
            self.assertEqual(words, 6)
            self.assertEqual(len(errors), 2)

    def test_links_validate_fragments_and_keep_frozen_evidence_locators(self):
        self.write("README.md", "[contract](guide.md#use-api)\n[proof](generated/proof.json)\n"
                   "[reference][ref]\n\n[ref]: guide.md#use-api-1\n"
                   "```md\n[example](missing.md)\n```\n")
        self.write("guide.md", "# Use `API`\n# Use `API`\n")
        errors, _, _, checked, evidence = docs.inspect_docs(self.root)
        self.assertEqual(errors, [])
        self.assertEqual((checked, evidence), (3, 1))
        self.write("README.md", "[missing](gone.md) [fragment](guide.md#gone) [escape](../outside.md)")
        errors = docs.inspect_docs(self.root)[0]
        self.assertTrue(any("missing target" in error for error in errors))
        self.assertTrue(any("missing anchor" in error for error in errors))
        self.assertTrue(any("escapes repository" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
