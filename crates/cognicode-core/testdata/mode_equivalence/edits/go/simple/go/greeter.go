package demo

// A comment-only and body-only edit.
func Greet(name string) string {
	return "hello, " + name
}

func Shout(name string) string {
	return Greet(name) + "!"
}

type Greeter struct {
	Prefix string
}

func (g Greeter) Say(name string) string {
	return g.Prefix + " " + name
}
