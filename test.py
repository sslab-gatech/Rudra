#!/usr/bin/env python3
import subprocess
import tempfile
import tomlkit
import traceback
import os
import os.path
import sys

from multiprocessing.pool import ThreadPool

print("Make sure you've installed the latest Rudra with `install-[debug|release].sh`")


class TestCase:
    PREFIX = b"/*!\n```rudra-test\n"

    def __init__(self, path):
        self.path = path
    
    @classmethod
    def create_test_case(cls, path):
        with open(path, "rb") as f:
            prefix = f.read(len(TestCase.PREFIX))
            if prefix != TestCase.PREFIX:
                return None
            return cls(path)

    def metadata(self):
        with open(self.path) as poc_file:
            lines = poc_file.readlines()
            # Two line prefix:
            # /*!
            # ```rudra-test
            # <parse this portion as toml>
            # ````
            idx = lines.index("```\n")
            toml_str = ''.join(lines[2:idx])
            return tomlkit.loads(toml_str)

    def __repr__(self):
        return "TestCase(%s)" % self.path


class TestResult:
    TEST_TYPES = ("normal", "fp")

    def __init__(self, test_case, test_type, failure=None):
        assert test_type in TestResult.TEST_TYPES
        self.test_case = test_case
        self.test_type = test_type
        self.failure = failure

    def is_success(self):
        return self.failure is None

    def __str__(self):
        if self.is_success():
            if self.test_type == "normal":
                return "\u001b[32;1mSUCCESS       \u001b[0m  {}".format(self.test_case.path)
            elif self.test_type == "fp":
                return "\u001b[33;1mFALSE-POSITIVE\u001b[0m  {}".format(self.test_case.path)
            elif self.test_type == "fn":
                return "\u001b[33;1mFALSE-NEGATIVE\u001b[0m  {}".format(self.test_case.path)
            else:
                raise Exception("Unknown test_type {}".format(self.test_type))
        else:
            return "\u001b[31;1mFAIL          \u001b[0m  {}\n{}".format(self.test_case.path, self.failure)


def run_test(test_case):
    metadata = test_case.metadata()
    test_type = metadata["test_type"]
    try:
        with tempfile.NamedTemporaryFile(prefix="rudra") as report_file:
            env_dict = dict(os.environ)
            env_dict["RUDRA_REPORT_PATH"] = report_file.name
            output = subprocess.run(
                [
                    "rudra",
                    "-Zrudra-enable-unsafe-destructor",
                    "--crate-type",
                    "lib",
                    test_case.path
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                env=env_dict,
                check=True,
            )
            with open(report_file.name) as report_file_handle:
                reports = tomlkit.loads(report_file_handle.read())
            expected_analyzers = set(metadata["expected_analyzers"])
            if "reports" in reports:
                reported_analyzers = set(map(lambda report: report["analyzer"], reports["reports"]))
            else:
                reported_analyzers = set()

            analyzer_mismatch_msg = "Analyzer set mismatch; expected {}, reported {}".format(
                list(expected_analyzers), list(reported_analyzers)
            )
            assert expected_analyzers == reported_analyzers, analyzer_mismatch_msg

            return TestResult(test_case, test_type)
    except (AssertionError,) as e:
        return TestResult(test_case, test_type, e)


total_cnt = {
    "normal": 0,
    "fp": 0,
}
success_cnt = {
    "normal": 0,
    "fp": 0,
}
def handle_result(test_result):
    global total_cnt, success_cnt
    total_cnt[test_result.test_type] += 1
    if test_result.is_success():
        success_cnt[test_result.test_type] += 1
    print(str(test_result))


files = [os.path.join(dp, f) for dp, dn, fn in os.walk("tests") for f in fn]

test_cases = filter(
    lambda t: t is not None,
    map(lambda path: TestCase.create_test_case(path), files)
)

if __name__ == "__main__":
    with ThreadPool(16) as pool:
        results = []
        for test_case in test_cases:
            results.append(pool.apply_async(run_test, (test_case,), callback=handle_result))
        
        for result in results:
            result.get()

    print("False-positives: {}/{}".format(success_cnt["fp"], total_cnt["fp"]))
    print("Normal: {}/{}".format(success_cnt["normal"], total_cnt["normal"]))

    if success_cnt["fp"] == total_cnt["fp"] and success_cnt["normal"] == total_cnt["normal"]:
        sys.exit(0)
    else:
        sys.exit(-1)
