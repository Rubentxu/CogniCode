#!/usr/bin/env bash
# scripts/ci/helm-lane.sh
#
# The faithful-render lane: the Helm tests that drive the real `helm template`
# engine, run with a PINNED helm and with COGNICODE_REQUIRE_HELM=1, so a missing
# or wrong binary fails the lane instead of skipping it.
#
# Without this lane the real-engine tests print SKIPPED and pass on a machine
# that has no helm, which certifies nothing about the render.
#
# The pin is the `helm` line of .tool-versions (asdf, the same convention that
# pins the rest of the toolchain). helm is installed at user level through asdf
# and never system-wide. asdf's helm plugin does not verify what it downloads,
# so the lane also checks the binary against the sha256 pinned below.
#
# Bump both together: the version in .tool-versions and the checksum of the
# `linux-amd64/helm` binary inside the tarball published at
# https://get.helm.sh/helm-v<version>-linux-amd64.tar.gz (its .sha256sum file
# covers the tarball, so extract it and hash the binary).
#
# Usage: bash scripts/ci/helm-lane.sh
set -euo pipefail

HELM_LINUX_AMD64_SHA256="46aeab5e043f62d01f7c168823b392cabc1cea3807b67906296290483791be43"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${REPO_ROOT}"

fail() {
  printf 'FAIL: helm-lane: %s\n' "$*" >&2
  exit 1
}

PIN="$(awk '$1 == "helm" { print $2; exit }' .tool-versions)"
[ -n "${PIN}" ] || fail "no 'helm <version>' line in .tool-versions"

command -v asdf >/dev/null 2>&1 || fail "asdf is not installed; it pins and installs helm for this lane"

if ! asdf plugin list | grep -qx helm; then
  asdf plugin add helm
fi
if ! HELM_DIR="$(asdf where helm "${PIN}" 2>/dev/null)" || [ ! -x "${HELM_DIR}/bin/helm" ]; then
  asdf install helm "${PIN}"
  HELM_DIR="$(asdf where helm "${PIN}")"
fi
HELM_BIN="${HELM_DIR}/bin/helm"
[ -x "${HELM_BIN}" ] || fail "asdf installed helm ${PIN} but ${HELM_BIN} is not executable"

GOT_VERSION="$("${HELM_BIN}" version --template '{{.Version}}')" || fail "${HELM_BIN} does not run"
[ "${GOT_VERSION}" = "v${PIN}" ] || fail "${HELM_BIN} is ${GOT_VERSION}, the pin is v${PIN}"

if [ "$(uname -s)-$(uname -m)" = "Linux-x86_64" ]; then
  GOT_SHA="$(sha256sum "${HELM_BIN}" | awk '{ print $1 }')"
  [ "${GOT_SHA}" = "${HELM_LINUX_AMD64_SHA256}" ] \
    || fail "${HELM_BIN} has sha256 ${GOT_SHA}, expected ${HELM_LINUX_AMD64_SHA256}"
fi

export PATH="${HELM_DIR}/bin:${PATH}"
export COGNICODE_REQUIRE_HELM=1
printf '=== helm-lane: %s (%s), COGNICODE_REQUIRE_HELM=1 ===\n' "${GOT_VERSION}" "${HELM_BIN}"

LOG="$(mktemp)"
trap 'rm -f "${LOG}"' EXIT
cargo test -p cognicode-core --test it -- helm_ --nocapture --test-threads=1 2>&1 | tee "${LOG}"

# Belt and braces: the lane is green only if the real-engine tests ran and none
# of them skipped.
if grep -q 'SKIPPED' "${LOG}"; then
  fail "a real-engine test skipped"
fi
for real_test in \
  helm_model::the_real_engine_renders_the_chart_when_it_is_installed \
  helm_pack::the_real_engine_feeds_the_pack_when_it_is_installed \
  helm_model::the_real_engine_agrees_with_the_double \
  mode_equivalence::helm_chart_rendered_by_the_real_engine_reaches_pass; do
  grep -q "test ${real_test}.* ok$" "${LOG}" || fail "${real_test} did not run and pass"
done
printf 'OK: helm-lane\n'
