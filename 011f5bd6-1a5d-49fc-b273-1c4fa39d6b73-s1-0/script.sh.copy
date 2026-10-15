test -f "/tmp/Cargo.toml" || {
    echo "FAIL: /tmp has no Cargo.toml"
    exit 1
}
test -f "/tmp/merge-gate.pipeline.kts" || {
    echo "FAIL: the merge authority merge-gate.pipeline.kts is missing from \/tmp."
    echo "      A pipeline that cannot find itself cannot gate anything."
    exit 1
}
test -d "/tmp/scripts/ci" || {
    echo "FAIL: \/tmp/scripts/ci is missing, so there is no contract suite to run"
    exit 1
}
echo "repo=/tmp authority=merge-gate.pipeline.kts crates=$(ls -d /tmp/crates/*/ | wc -l)"