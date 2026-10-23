#!/usr/bin/env bash
# check-merge-subject.sh — judge the header a squash merge is about to create.
#
# WHAT IT JUDGES
# ---------------
# A squash merge creates exactly one commit, and its header is the title of
# the pull request followed by the pull request's number: the forge composes
# it that way when the merge does not carry a subject of its own. That
# commit is created by the forge, on the forge's side, so it never passes
# through the hooks this machine runs on the commits it makes itself. The
# house policy therefore sees that header for the first time afterwards, when
# the machine tries to reference the new commit and the policy refuses the
# reference.
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
# Two rules that belong to this repository and not to the policy:
#
#   - the title of a pull request does not carry a pull request reference.
#     The forge adds it, and a title that already carries one would land a
#     header with the reference twice;
#   - the header that lands is the title plus that reference, so the title
#     has to leave room for it. A title that fits the limit on its own can
#     still produce a header that does not fit, and the title alone is not
#     what the house has to carry.
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
#   bash scripts/ci/check-merge-subject.sh --pr <number> [--repo <owner/name>]
#   bash scripts/ci/check-merge-subject.sh --sha <sha> [--repo <owner/name>]
#   bash scripts/ci/check-merge-subject.sh --pr <number> --title "<title>"
#   bash scripts/ci/check-merge-subject.sh --header "<header>"
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
  check-merge-subject.sh --pr <number> [--title "<title>"] [--repo <owner/name>]
  check-merge-subject.sh --sha <sha> [--repo <owner/name>]
  check-merge-subject.sh --header "<header>"

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
title=""
pull_request=""
repo=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --pr)
            [ "$#" -ge 2 ] || die "--pr needs the number of the pull request."
            mode="pull-request"; pull_request="$2"; shift 2;;
        --sha)
            [ "$#" -ge 2 ] || die "--sha needs the commit."
            mode="commit"; header="$2"; shift 2;;
        --title)
            [ "$#" -ge 2 ] || die "--title needs the title of the pull request."
            title="$2"; shift 2;;
        --header)
            [ "$#" -ge 2 ] || die "--header needs the header to judge."
            mode="header"; header="$2"; shift 2;;
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
    die "one of --pr, --sha or --header is required."
fi

if [ "$mode" = "header" ]; then
    if [ -n "$title" ] || [ -n "$pull_request" ]; then
        die "--header judges a header as it is; the title of a pull request is judged with --pr."
    fi
else
    if [ -z "$repo" ]; then
        origin="$(git remote get-url origin 2>/dev/null || true)"
        repo="$(printf '%s' "$origin" | sed -E 's#^git@[^:]+:##; s#^https?://[^/]+/##; s#\.git$##')"
        if [ -z "$repo" ] || [ "$repo" = "$origin" ]; then
            die "cannot work out owner/name from the origin remote ($origin). Pass --repo."
        fi
    fi

    command -v gh >/dev/null 2>&1 \
        || die "the \`gh\` CLI is not on PATH, so the title of the pull request cannot be read."

    if [ "$mode" = "commit" ]; then
        answer="$(gh api "repos/$repo/commits/$header/pulls" --jq '.[0] | "\(.number)\t\(.title)"')" \
            || die "cannot read the pull requests of $header in $repo."
        if [ -z "$answer" ]; then
            echo "==> no pull request holds this commit: there is no squash header to judge"
            exit 0
        fi
        pull_request="${answer%%$'\t'*}"
        title="${answer#*$'\t'}"
    elif [ -z "$title" ]; then
        answer="$(gh api "repos/$repo/pulls/$pull_request" --jq '"\(.number)\t\(.title)"')" \
            || die "cannot read the pull request $pull_request in $repo."
        if [ -z "$answer" ]; then
            die "the pull request $pull_request in $repo came back without a title; there is nothing to judge."
        fi
        title="${answer#*$'\t'}"
    fi

    if [ -z "$title" ]; then
        die "the title of the pull request is empty; there is nothing to judge."
    fi

    # The title does not carry the reference: the forge adds it, and a title
    # that already carries one would land a header with it twice.
    case "$title" in
        *" (#"[0-9]*")")
            echo "ERROR: the title already carries a pull request reference." >&2
            echo "       The forge adds it to the header, so the header would carry it" >&2
            echo "       twice. The title of a pull request does not carry it." >&2
            echo "       title: $title" >&2
            exit 1;;
    esac

    header="$title (#$pull_request)"
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

echo "==> the header is admitted"
