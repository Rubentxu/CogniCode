#!/usr/bin/env bash
# check-merge-subject.sh — judge the header a squash merge is about to create.
#
# WHAT IT JUDGES
# ---------------
# A squash merge creates exactly one commit, and its header is the title of
# the pull request, verbatim. That commit is created by the forge, on the
# forge's side, so it never passes through the hooks this machine runs on
# the commits it makes itself. The house policy therefore sees that header
# for the first time afterwards, when the machine tries to reference the new
# commit and the policy refuses the reference.
#
# By then the merge is public, the branch does not accept a rewritten
# history, and the repository carries a commit that its own policy will not
# let a clone point at. The mistake costs minutes of gate time after the
# fact and cannot be undone afterwards.
#
# So the header is judged before the merge, by the same authority that
# judges every other commit this house makes: `git-control check --message`.
# The limit and the list of admitted commit types live in that authority's
# policy, and this script keeps no copy of them on purpose. A copy would be
# a second truth, and the two would drift apart until one of them admitted
# exactly what the other forbids.
#
# WHAT IT ADDS TO THE POLICY
# --------------------------
# One rule that belongs to this repository and not to the policy: the header
# of a squash merge closes the pull request it comes from, and says so, the
# way every other commit in this history does. A title without it would be
# admitted by the policy and would land a commit that does not say where it
# came from.
#
# WHAT IT DOES NOT DO
# -------------------
# It does not merge anything, and it does not decide whether the change is
# good. It answers one question — would this header be admitted? — and it
# answers it with the policy of the house.
#
# EXIT CODES
# ----------
#   0  the header is admitted
#   1  the header is refused
#   2  the header could not be judged: no authority, no answer from the
#      forge, or no such pull request to read
#
# Usage:
#   bash scripts/ci/check-merge-subject.sh --title "<header>"
#   bash scripts/ci/check-merge-subject.sh --pr <number> [--repo <owner/name>]
#   bash scripts/ci/check-merge-subject.sh --sha <sha> [--repo <owner/name>]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# The authority that decides which headers this house admits. It is a
# machine-level tool on purpose: the policy it reads is the policy the
# commit hooks of this machine enforce, so the two cannot disagree.
AUTHORITY="git-control"

usage() {
    cat <<'USAGE'
Usage:
  check-merge-subject.sh --title "<header>"
  check-merge-subject.sh --pr <number> [--repo <owner/name>]
  check-merge-subject.sh --sha <sha> [--repo <owner/name>]

Judges the header a squash merge would create, using the policy of the house.
Exit codes: 0 admitted, 1 refused, 2 could not be judged.
USAGE
}

die() {
    echo "ERROR: $*" >&2
    exit 2
}

mode=""
header=""
pull_request=""
repo=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --title)
            [ "$#" -ge 2 ] || die "--title needs the header to judge."
            mode="title"; header="$2"; shift 2;;
        --pr)
            [ "$#" -ge 2 ] || die "--pr needs the number of the pull request."
            mode="pull-request"; pull_request="$2"; shift 2;;
        --sha)
            [ "$#" -ge 2 ] || die "--sha needs the commit."
            mode="commit"; header="$2"; shift 2;;
        --repo)
            [ "$#" -ge 2 ] || die "--repo needs owner/name."
            repo="$2"; shift 2;;
        -h|--help)
            usage; exit 0;;
        *)
            usage >&2
            die "unknown argument: $1";;
    esac
done

if [ -z "$mode" ]; then
    usage >&2
    die "one of --title, --pr or --sha is required."
fi

if [ "$mode" != "title" ]; then
    if [ -z "$repo" ]; then
        origin="$(git remote get-url origin 2>/dev/null || true)"
        repo="$(printf '%s' "$origin" | sed -E 's#^git@[^:]+:##; s#^https?://[^/]+/##; s#\.git$##')"
        if [ -z "$repo" ] || [ "$repo" = "$origin" ]; then
            die "cannot work out owner/name from the origin remote ($origin). Pass --repo."
        fi
    fi
    command -v gh >/dev/null 2>&1 \
        || die "the \`gh\` CLI is not on PATH, so the title of the pull request cannot be read."

    if [ "$mode" = "pull-request" ]; then
        answer="$(gh api "repos/$repo/pulls/$pull_request" --jq '"\(.number)\t\(.title)"')" \
            || die "cannot read the pull request $pull_request in $repo."
    else
        answer="$(gh api "repos/$repo/commits/$header/pulls" --jq '.[0] | "\(.number)\t\(.title)"')" \
            || die "cannot read the pull requests of $header in $repo."
    fi

    if [ -z "$answer" ]; then
        echo "==> no pull request holds this commit: there is no squash header to judge"
        exit 0
    fi
    pull_request="${answer%%$'\t'*}"
    header="${answer#*$'\t'}"
fi

if [ -z "$header" ]; then
    die "the title of the pull request came back empty; there is nothing to judge."
fi

command -v "$AUTHORITY" >/dev/null 2>&1 \
    || die "the \`$AUTHORITY\` binary is not on PATH. It is the authority that decides which headers this house admits, and this script does not keep a copy of its rules."

echo "==> judging the header a squash merge would create"
echo "    header: $header"

verdict=0
"$AUTHORITY" check --message "$header" || verdict=$?

if [ "$verdict" -ne 0 ]; then
    echo "==> the header is refused: the merge would create a commit this house cannot carry" >&2
    exit 1
fi

if [ -n "$pull_request" ]; then
    case "$header" in
        *" (#$pull_request)")
            ;;
        *)
            echo "ERROR: the header does not close the pull request it belongs to." >&2
            echo "       A squash merge takes the title verbatim, so the title has to end" >&2
            echo "       with ' (#$pull_request)'." >&2
            echo "       header: $header" >&2
            exit 1;;
    esac
fi

echo "==> the header is admitted"
