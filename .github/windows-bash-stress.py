"""Isolated diagnostic branch only: keep every failed burst assertion."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess
import sys

TEST = "tty::windows::test::bash_input::git_bash_wrapped_input_can_be_fully_erased_in_byte_and_utf8_locales"
binaries = list(Path("target/ci/deps").glob("nebula_terminal-*.exe"))
if len(binaries) != 1:
    raise SystemExit(f"Expected one native test binary, found {binaries}")


def run_one(iteration):
    result = subprocess.run(
        [str(binaries[0].resolve()), TEST, "--exact", "--nocapture"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        encoding="utf-8", errors="replace", timeout=90,
    )
    return iteration, result.returncode, result.stdout


failures = 0
with ThreadPoolExecutor(max_workers=4) as executor:
    for iteration, code, output in executor.map(run_one, range(1, 25)):
        print(f"{sys.argv[1]} concurrent iteration {iteration}: exit {code}", flush=True)
        print(output, flush=True)
        failures += code != 0
print(f"{sys.argv[1]} failures: {failures} / 24", flush=True)
raise SystemExit(bool(failures))
