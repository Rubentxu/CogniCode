// SystemVerilog test fixture
module main;
    int result;

    function int compute(int x);
        return x * 2;
    endfunction

    initial begin
        result = compute(42);
        $display("Result: %0d", result);
    end
endmodule
