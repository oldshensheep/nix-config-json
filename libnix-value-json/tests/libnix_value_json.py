import argparse
import csv
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class LazyToJsonCase(unittest.TestCase):
    def __init__(
        self, plugin: str, store_url: str, index: int, case: dict[str, str]
    ) -> None:
        super().__init__("runTest")
        self.plugin = plugin
        self.store_url = store_url
        self.name = f"{index:03d} {case['name']}"
        self.expr = case["expr"]
        self.expected = case["expected"]
        self.expected_error = case.get("expected_error") or ""

    def __str__(self) -> str:
        return self.name

    def shortDescription(self) -> None:
        return None

    def nix_eval(self, expr: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "nix",
                "--extra-experimental-features",
                "nix-command",
                "--store",
                self.store_url,
                "eval",
                "--impure",
                "--plugin-files",
                self.plugin,
                "--raw",
                "--expr",
                expr,
            ],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

    def assert_eval_succeeds(self) -> None:
        result = self.nix_eval(self.expr)
        if result.returncode != 0:
            self.fail(
                "nix eval failed\n"
                f"expression: {self.expr}\n"
                    f"stderr:\n{result.stderr.strip()}"
            )
        self.assertEqual(result.stdout, self.expected)

    def assert_eval_fails(self) -> None:
        result = self.nix_eval(self.expr)
        if result.returncode == 0:
            self.fail(
                "nix eval succeeded unexpectedly\n"
                f"expression: {self.expr}\n"
                f"stdout:\n{result.stdout}"
            )
        self.assertIn(self.expected_error, result.stderr)

    def runTest(self) -> None:
        if self.expected_error:
            self.assert_eval_fails()
        else:
            self.assert_eval_succeeds()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("plugin", help="path to libnix-value-json.so")
    parser.add_argument(
        "--cases",
        type=Path,
        default=Path(__file__).with_suffix(".csv"),
        help="path to CSV file containing name, expr, and expected columns",
    )
    return parser.parse_args()


def load_cases(cases_path: Path) -> list[dict[str, str]]:
    with cases_path.open(newline="") as cases_file:
        reader = csv.DictReader(cases_file)
        required_columns = {"name", "expr"}
        missing_columns = required_columns - set(reader.fieldnames or [])
        if missing_columns:
            missing = ", ".join(sorted(missing_columns))
            raise SystemExit(f"{cases_path} is missing required column(s): {missing}")

        cases = list(reader)

    if not cases:
        raise SystemExit(f"no test cases found in {cases_path}")

    return cases


def make_suite(plugin: str, store_url: str, cases_path: Path) -> unittest.TestSuite:
    return unittest.TestSuite(
        LazyToJsonCase(plugin, store_url, index, expand_case_paths(case, cases_path.parent))
        for index, case in enumerate(load_cases(cases_path), 1)
    )


def expand_case_paths(case: dict[str, str], cases_dir: Path) -> dict[str, str]:
    expanded = dict(case)
    test_dir = str(cases_dir.resolve())
    expanded["expr"] = (expanded.get("expr") or "").replace("@TEST_DIR@", test_dir)
    expanded["expected"] = (expanded.get("expected") or "").replace(
        "@TEST_DIR@", test_dir
    )
    expanded["expected_error"] = (expanded.get("expected_error") or "").replace(
        "@TEST_DIR@", test_dir
    )
    return expanded


def main() -> int:
    args = parse_args()
    with tempfile.TemporaryDirectory(prefix="nix-value-json-test-") as temp_dir:
        store_url = str(Path(temp_dir) / "store")
        suite = make_suite(args.plugin, store_url, args.cases)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
