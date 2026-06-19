// Zig test fixture
const std = @import("std");

fn compute(x: i32) i32 {
    return x * 2;
}

fn greet(name: []const u8) !void {
    const stdout = std.io.getStdOut().writer();
    try stdout.print("Hello, {s}
", .{name});
}

pub fn main() !void {
    const result = compute(42);
    try greet("world");
    _ = result;
}
