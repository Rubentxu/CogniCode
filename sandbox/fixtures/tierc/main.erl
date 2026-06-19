%% Erlang test fixture
-module(main).
-export([main/0]).

compute(X) -> X * 2.

greet(Name) -> io:format("Hello, ~s~n", [Name]).

main() ->
    Result = compute(42),
    greet("world"),
    io:format("~p~n", [Result]).
