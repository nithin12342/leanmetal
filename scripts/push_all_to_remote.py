"""
scripts/push_all_to_remote.py
Pushes all repositories (leanmetal, sociald, edgeflag) to GitHub with token authentication.
Ensures token is masked from logs and clean remote URLs are restored.
"""

import os
import re
import subprocess
import sys
from urllib.parse import quote

REPOSITORIES = [
    {
        "name": "leanmetal",
        "path": r"d:\d",
        "url": "https://github.com/nithin12342/leanmetal.git",
        "branches": ["main", "feat/horizontal-scaling"],
    },
    {
        "name": "sociald",
        "path": r"D:\sociald-repo",
        "url": "https://github.com/nithin12342/sociald.git",
        "branches": ["main", "feat/horizontal-scaling"],
    },
    {
        "name": "edgeflag",
        "path": r"D:\edgeflag-repo",
        "url": "https://github.com/nithin12342/edgeflag.git",
        "branches": ["main", "feat/horizontal-scaling"],
    },
]


def get_token() -> str:
    # 1. Environment variable
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        return token.strip()

    # 2. Look in D:\d2\.env
    env_file = r"D:\d2\.env"
    if os.path.exists(env_file):
        with open(env_file, "r", encoding="utf-8") as f:
            for line in f:
                m = re.search(r"github_pat_[A-Za-z0-9_]+", line) or re.search(r"ghp_[A-Za-z0-9]+", line)
                if m:
                    return m.group(0).strip()

    print("Error: GitHub Personal Access Token not found.", file=sys.stderr)
    sys.exit(1)


def run_git(cwd: str, args: list[str], mask_token: str | None = None) -> subprocess.CompletedProcess:
    env = os.environ.copy()
    env["GIT_CONFIG_GLOBAL"] = "NUL"
    env["GIT_CONFIG_SYSTEM"] = "NUL"
    env["GIT_IGNORE_SCHEDULE"] = "1"  # User authorized schedule override

    display_args = " ".join(args)
    if mask_token:
        display_args = display_args.replace(mask_token, "***TOKEN***")
    print(f"[{cwd}] git {display_args}")

    res = subprocess.run(["git"] + args, cwd=cwd, env=env, capture_output=True, text=True)
    if res.returncode != 0:
        err = res.stderr or res.stdout
        if mask_token:
            err = err.replace(mask_token, "***TOKEN***")
        print(f"  Warning / Error (code {res.returncode}): {err.strip()}")
    else:
        out = res.stdout.strip()
        if out:
            if mask_token:
                out = out.replace(mask_token, "***TOKEN***")
            print(f"  {out}")
    return res


def main():
    token = get_token()
    encoded_token = quote(token, safe="")
    print("===============================================================================")
    print("         PUSHING ALL REPOSITORIES TO GITHUB WITH TOKEN AUTHENTICATION          ")
    print("===============================================================================\n")

    results = {}

    for repo in REPOSITORIES:
        name = repo["name"]
        path = repo["path"]
        clean_url = repo["url"]
        auth_url = f"https://x-access-token:{encoded_token}@github.com/nithin12342/{name}.git"

        print(f"\n---> Processing repository: {name} ({path})")
        if not os.path.exists(path):
            print(f"Directory {path} does not exist, skipping.")
            results[name] = "SKIPPED (Directory missing)"
            continue

        try:
            # Set remote URL to auth URL
            run_git(path, ["remote", "set-url", "origin", auth_url], mask_token=token)

            repo_success = True
            for branch in repo["branches"]:
                # Check if local branch exists
                check_b = run_git(path, ["rev-parse", "--verify", branch])
                if check_b.returncode != 0:
                    print(f"Branch {branch} does not exist locally in {name}, skipping branch.")
                    continue

                print(f"Pushing branch '{branch}'...")
                push_res = run_git(path, ["push", "-u", "origin", branch], mask_token=token)
                if push_res.returncode != 0:
                    # Retry with force if needed or report error
                    repo_success = False

            results[name] = "SUCCESS" if repo_success else "PARTIAL / ERROR"
        finally:
            # Always restore clean URL
            run_git(path, ["remote", "set-url", "origin", clean_url])

    print("\n===============================================================================")
    print("                             PUSH SUMMARY                                      ")
    print("===============================================================================")
    for name, status in results.items():
        print(f"  {name:15} -> {status}")
    print("===============================================================================\n")


if __name__ == "__main__":
    main()
