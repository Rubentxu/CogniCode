// Verilog test fixture
module main;
    integer result;

    function integer compute;
        input integer x;
        begin
            compute = x * 2;
        end
    endfunction

    initial begin
        result = compute(42);
        $display("Result: %0d", result);
    end
endmodule
