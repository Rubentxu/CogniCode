package demo

func Greet(name string) string {
	return "hello " + name
}

func Yell(name string) string {
	return Greet(name) + "!"
}

type Greeter struct {
	Prefix string
}

func (g Greeter) Say(name string) string {
	return g.Prefix + " " + name
}

func Count(names []string) int {
	return len(names)
}
