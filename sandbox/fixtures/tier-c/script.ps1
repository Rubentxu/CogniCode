# PowerShell test fixture
function Compute($x) { return $x * 2 }
function Greet($name) { Write-Output "Hello, $name" }
$result = Compute(42)
Greet("world")
Write-Output $result
