fun stage(name: String, body: () -> Unit) {
    println("stage $name")
    body()
}

stage("build") { println("gradle build") }
stage("test") { println("gradle test") }
