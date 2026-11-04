// Jenkins shared-library step: `notify('done')`.
def call(String message) {
    echo "notify: ${message}"
}

def failed(String reason) {
    call("failed: ${reason}")
}
