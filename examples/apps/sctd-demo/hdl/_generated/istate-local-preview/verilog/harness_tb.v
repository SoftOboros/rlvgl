// Generated iState Verilog testbench
// Reads events from vectors/events.txt and outputs trace
`timescale 1ns/1ps


module SctdPhilosophersLedTop_tb;

// Clock period (ns)
localparam CLK_PERIOD = 10;

// State encoding
localparam [3:0] STATE_IDLERELEASED = 0;
localparam [3:0] STATE_DEBOUNCEPRESS = 1;
localparam [3:0] STATE_DEBOUNCERELEASE = 2;
localparam [3:0] STATE_THINKALL = 3;
localparam [3:0] STATE_HUNGRYODD = 4;
localparam [3:0] STATE_EATODD = 5;
localparam [3:0] STATE_HUNGRYEVEN = 6;
localparam [3:0] STATE_EATEVEN = 7;
localparam [3:0] STATE_HUNGRYFIVE = 8;
localparam [3:0] STATE_EATFIVE = 9;

// Event encoding
localparam [1:0] EVENT_BUTTON_HIGH = 0;
localparam [1:0] EVENT_BUTTON_LOW = 1;
localparam [1:0] EVENT_TICK_12MHZ = 2;

// DUT signals
reg clk;
reg rst;
reg event_valid;
reg [1:0] event_code;
wire [3:0] state_out;
wire transition_taken;
wire signed [31:0] dm_timer_count;
wire signed [31:0] dm_phase_ms;
wire signed [31:0] dm_debounce_ms;
wire signed [31:0] dm_p1_r;
wire signed [31:0] dm_p1_g;
wire signed [31:0] dm_p1_b;
wire signed [31:0] dm_p2_r;
wire signed [31:0] dm_p2_g;
wire signed [31:0] dm_p2_b;
wire signed [31:0] dm_p3_r;
wire signed [31:0] dm_p3_g;
wire signed [31:0] dm_p3_b;
wire signed [31:0] dm_p4_r;
wire signed [31:0] dm_p4_g;
wire signed [31:0] dm_p4_b;
wire signed [31:0] dm_p5_r;
wire signed [31:0] dm_p5_g;
wire signed [31:0] dm_p5_b;

// Test variables
reg [3:0] prev_state;
integer events_file;
integer output_file;
reg [255:0] event_str;
integer scan_result;

// DUT instantiation
SctdPhilosophersLedTop_fsm dut (
    .clk(clk),
    .rst(rst),
    .event_valid(event_valid),
    .event_code(event_code),
    .state_out(state_out),
    .transition_taken(transition_taken)
    ,.dm_timer_count(dm_timer_count)
    ,.dm_phase_ms(dm_phase_ms)
    ,.dm_debounce_ms(dm_debounce_ms)
    ,.dm_p1_r(dm_p1_r)
    ,.dm_p1_g(dm_p1_g)
    ,.dm_p1_b(dm_p1_b)
    ,.dm_p2_r(dm_p2_r)
    ,.dm_p2_g(dm_p2_g)
    ,.dm_p2_b(dm_p2_b)
    ,.dm_p3_r(dm_p3_r)
    ,.dm_p3_g(dm_p3_g)
    ,.dm_p3_b(dm_p3_b)
    ,.dm_p4_r(dm_p4_r)
    ,.dm_p4_g(dm_p4_g)
    ,.dm_p4_b(dm_p4_b)
    ,.dm_p5_r(dm_p5_r)
    ,.dm_p5_g(dm_p5_g)
    ,.dm_p5_b(dm_p5_b)
);

// Clock generation
initial begin
    clk = 0;
    forever #(CLK_PERIOD/2) clk = ~clk;
end

// State to string function (for display)
function [127:0] state_to_string;
    input [3:0] s;
    begin
        case (s)
            STATE_IDLERELEASED: state_to_string = "IdleReleased";
            STATE_DEBOUNCEPRESS: state_to_string = "DebouncePress";
            STATE_DEBOUNCERELEASE: state_to_string = "DebounceRelease";
            STATE_THINKALL: state_to_string = "ThinkAll";
            STATE_HUNGRYODD: state_to_string = "HungryOdd";
            STATE_EATODD: state_to_string = "EatOdd";
            STATE_HUNGRYEVEN: state_to_string = "HungryEven";
            STATE_EATEVEN: state_to_string = "EatEven";
            STATE_HUNGRYFIVE: state_to_string = "HungryFive";
            STATE_EATFIVE: state_to_string = "EatFive";
            default: state_to_string = "(unknown)";
        endcase
    end
endfunction

// Event string to code function
function [1:0] event_from_string;
    input [255:0] s;
    begin
        if (s == "button_high") event_from_string = EVENT_BUTTON_HIGH;
        else         if (s == "button_low") event_from_string = EVENT_BUTTON_LOW;
        else         if (s == "tick_12mhz") event_from_string = EVENT_TICK_12MHZ;
        else event_from_string = {(2){1'b1}}; // Invalid
    end
endfunction

// Trim string (remove trailing whitespace/newlines)
function [255:0] trim_string;
    input [255:0] s;
    integer i;
    reg [7:0] c;
    begin
        trim_string = s;
        for (i = 0; i < 32; i = i + 1) begin
            c = s[i*8 +: 8];
            if (c == 8'h0A || c == 8'h0D || c == 8'h20 || c == 8'h00) begin
                trim_string[i*8 +: 8] = 8'h00;
            end
        end
    end
endfunction

// Main test sequence
initial begin
    // Initialize
    rst = 1;
    event_valid = 0;
    event_code = 0;

    // Open output file
    output_file = $fopen("output.trace.txt", "w");
    if (output_file == 0) begin
        $display("ERROR: Cannot open output.trace.txt");
        $finish;
    end

    // Reset sequence
    repeat(2) @(posedge clk);
    rst = 0;
    @(posedge clk);

    // Output initial state entry
    $fwrite(output_file, "on_entry:%s\n", state_to_string(state_out));
    $display("on_entry:%s", state_to_string(state_out));

    // Open events file
    events_file = $fopen("vectors/events.txt", "r");
    if (events_file == 0) begin
        $display("ERROR: Cannot open vectors/events.txt");
        $fclose(output_file);
        $finish;
    end

    // Process each event
    while (!$feof(events_file)) begin
        scan_result = $fscanf(events_file, "%s\n", event_str);
        if (scan_result == 1) begin
            event_str = trim_string(event_str);
            if (event_str != 0) begin
                prev_state = state_out;

                // Apply event
                event_code = event_from_string(event_str);
                event_valid = 1;
                @(posedge clk);
                event_valid = 0;

                // Check result after clock edge
                @(negedge clk);

                if (transition_taken) begin
                    $fwrite(output_file, "on_exit:%s\n", state_to_string(prev_state));
                    $display("on_exit:%s", state_to_string(prev_state));

                    $fwrite(output_file, "transition:%s->%s\n", state_to_string(prev_state), state_to_string(state_out));
                    $display("transition:%s->%s", state_to_string(prev_state), state_to_string(state_out));

                    $fwrite(output_file, "on_entry:%s\n", state_to_string(state_out));
                    $display("on_entry:%s", state_to_string(state_out));
                end else begin
                    $fwrite(output_file, "no_transition:%s on %s\n", state_to_string(prev_state), event_str);
                    $display("no_transition:%s on %s", state_to_string(prev_state), event_str);
                end

                @(posedge clk);
            end
        end
    end

    // Cleanup
    $fclose(events_file);
    $fclose(output_file);

    $display("Simulation complete. Check output.trace.txt");
    $finish;
end

endmodule