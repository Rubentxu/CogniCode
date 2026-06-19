-- Haskell test fixture
module Main where

compute :: Int -> Int
compute x = x * 2

greet :: String -> IO ()
greet name = putStrLn ("Hello, " ++ name)

main :: IO ()
main = do
    let result = compute 42
    greet "world"
    print result
