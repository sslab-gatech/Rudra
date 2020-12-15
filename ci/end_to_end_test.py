#!/usr/bin/env python3
import subprocess
import tempfile
import tomlkit
import os
import sys


def download_crate(crate_name, version):
    with tempfile.NamedTemporaryFile(prefix="rudra", suffix=".tar.gz") as f:
        subprocess.run(['cargo', 'download', '%s==%s' % (crate_name, version)], stdout=f)
        return subprocess.check_output(['tar', 'xvf', f.name]).decode('ascii').split('\n')[0].split('/')[0]


def run_rudra(crate_name, crate_path):
    with tempfile.NamedTemporaryFile(prefix="rudra") as report_file:
        env_dict = dict(os.environ)
        env_dict["RUDRA_REPORT_PATH"] = report_file.name
        output = subprocess.run(
            ["sh", "-c", "cd %s ; cargo rudra" % crate_path],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            env=env_dict,
            check=True,
        )
        with open(report_file.name + '-lib-' + crate_name) as report_file_handle:
            return tomlkit.loads(report_file_handle.read())['reports']


if __name__ == "__main__":
    missing_reports = []
    testcases = tomlkit.loads(open('ci/end_to_end_test.toml').read())
    for crate in testcases['crates']:
        rudra_reports = run_rudra(crate['name'], download_crate(crate['name'], crate['version']))
        rudra_reports_set = set()
        for rudra_report in rudra_reports:
            rudra_reports_set.add((rudra_report['analyzer'], rudra_report['location']))
        for expected_report in crate['expected_reports']:
            expected_report = tuple(expected_report)
            if expected_report not in rudra_reports_set:
                missing_reports.append((crate['name'], ) + expected_report)
    if missing_reports:
        print('MISSING REPORTS')
        for missing_report in missing_reports:
            print(missing_report)
        sys.exit(-1)
    else:
        print('SUCCESS')
        sys.exit(0)
