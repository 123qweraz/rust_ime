import subprocess
def test(py):
    res = subprocess.run(["./target/debug/rust-ime", "--test"], input=f"{py}\nexit\n", capture_output=True, text=True)
    print(f"--- {py} ---")
    for line in res.stdout.splitlines():
        if "1. " in line: print(line)
test("sm")
test("zm")
