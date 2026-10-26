// Un script de pipeline es Kotlin, pero es un SCRIPT: sus sentencias se
// ejecutan en orden. El producto lo lee como fuente Kotlin y no finge
// ejecutar el pipeline ni los servicios que el pipeline declara.
val pipelineName = "k1"
println(pipelineName)
