// Jenkins shared-library step: `deploy(manifest: 'k8s/app.yaml')`.
def call(Map config = [:]) {
    def kube = new org.acme.Kube(this)
    kube.apply(config.manifest)
}
