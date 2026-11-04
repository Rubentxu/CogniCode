package org.acme

class Kube implements Serializable {
    def steps

    Kube(steps) {
        this.steps = steps
    }

    def apply(String manifest) {
        steps.sh "kubectl apply -f ${manifest}"
    }

    def rollback(String name) {
        steps.sh "kubectl rollout undo ${name}"
    }
}
