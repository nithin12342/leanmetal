"""
Python script to push the local git repository to GitHub remote with token authentication.

Repository: https://github.com/nithin12342/leanmetal
Supports: GitHub Fine-Grained Personal Access Tokens (PATs) and classic tokens.
"""

import argparse
import getpass
import os
import subprocess
import sys
from urllib.parse import quote

REMOTE_REPO = "nithin12342/leanmetal"
DEFAULT_REMOTE_URL = f"https://github.com/{REMOTE_REPO}.git"


def run_cmd(cmd: list[str], mask_token: str | None = None, check: bool = True) -> subprocess.CompletedProcess:
    """Run a command and print output while masking sensitive tokens."""
    display_cmd = " ".join(cmd)
    if mask_token:
        display_cmd = display_cmd.replace(mask_token, "***TOKEN***")
    print(f"-> Running: {display_cmd}")

    res = subprocess.run(cmd, capture_output=True, text=True)
    if check and res.returncode != 0:
        err_msg = res.stderr or res.stdout
        if mask_token:
            err_msg = err_msg.replace(mask_token, "***TOKEN***")
        print(f"Error (code {res.returncode}):\n{err_msg}", file=sys.stderr)
        sys.exit(res.returncode)

    if res.stdout.strip():
        out = res.stdout
        if mask_token:
            out = out.replace(mask_token, "***TOKEN***")
        print(out.strip())
    return res


def get_current_branch() -> str:
    res = subprocess.run(
        ["git", "branch", "--show-current"], capture_output=True, text=True
    )
    branch = res.stdout.strip()
    return branch if branch else "master"


def main():
    parser = argparse.ArgumentParser(description="Push local git repository to GitHub remote.")
    parser.add_argument(
        "--token",
        dest="token",
        default=os.environ.get("GITHUB_TOKEN"),
        help="GitHub Personal Access Token (or set GITHUB_TOKEN env var)",
    )
    parser.add_argument(
        "--branch",
        dest="branch",
        default=None,
        help="Branch to push to (defaults to 'main')",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Force push to remote",
    )

    args = parser.parse_args()

    token = args.token
    if not token or token.startswith("http"):
        token = getpass.getpass("Enter GitHub Personal Access Token (PAT): ").strip()

    if not token:
        print("Error: A valid GitHub Personal Access Token is required.", file=sys.stderr)
        sys.exit(1)

    clean_url = f"https://github.com/{REMOTE_REPO}.git"

    # Configure origin remote
    remotes = subprocess.run(["git", "remote"], capture_output=True, text=True).stdout.split()
    if "origin" in remotes:
        run_cmd(["git", "remote", "set-url", "origin", clean_url])
    else:
        run_cmd(["git", "remote", "add", "origin", clean_url])

    # GitHub fine-grained PAT requires x-access-token as user
    encoded_token = quote(token, safe="")
    auth_url = f"https://x-access-token:{encoded_token}@github.com/{REMOTE_REPO}.git"

    # Standardize local branch to main if currently master
    current_branch = get_current_branch()
    target_branch = args.branch if args.branch else "main"

    if current_branch == "master" and target_branch == "main":
        print("[+] Renaming branch 'master' -> 'main'...")
        run_cmd(["git", "branch", "-M", "main"])
        current_branch = "main"

    try:
        # Check if remote has commits
        print(f"[+] Fetching remote tracking info from {clean_url}...")
        fetch_res = run_cmd(["git", "fetch", auth_url, target_branch], mask_token=token, check=False)
        
        if fetch_res.returncode == 0:
            print("[+] Merging existing remote history (e.g. LICENSE/README)...")
            run_cmd(["git", "merge", "FETCH_HEAD", "--allow-unrelated-histories", "-m", "Merge remote repository initial files"], check=False)

        print(f"[+] Pushing {current_branch}:{target_branch} to GitHub...")
        push_cmd = ["git", "push", "-u", auth_url, f"{current_branch}:{target_branch}"]
        if args.force:
            push_cmd.append("--force")
        run_cmd(push_cmd, mask_token=token)

        print("\n[SUCCESS] Successfully pushed all commits to GitHub!")
        print(f"Repository URL: https://github.com/{REMOTE_REPO}")
    finally:
        # Guarantee token is not saved in git config
        run_cmd(["git", "remote", "set-url", "origin", clean_url])


if __name__ == "__main__":
    main()
