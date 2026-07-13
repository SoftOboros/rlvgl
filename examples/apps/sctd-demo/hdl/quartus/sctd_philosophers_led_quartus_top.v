// sctd_philosophers_led_quartus_top.v
//
// Temporary Quartus-facing shim for the iState-generated SCTD philosophers
// LED proof. The chart behavior remains in SctdPhilosophersLedTop_fsm,
// generated from ../philosophers_led_top.scxml. This shim only adapts board
// pins to the current generic iState HDL interface.

module sctd_philosophers_led_quartus_top #(
    parameter BUTTON_ACTIVE_LOW = 1'b0,
    parameter LED_ACTIVE_LOW = 1'b0
) (
    input  wire clk_12mhz,
    input  wire rst,
    input  wire button_raw,
    output wire p1_r,
    output wire p1_g,
    output wire p1_b,
    output wire p2_r,
    output wire p2_g,
    output wire p2_b,
    output wire p3_r,
    output wire p3_g,
    output wire p3_b,
    output wire p4_r,
    output wire p4_g,
    output wire p4_b,
    output wire p5_r,
    output wire p5_g,
    output wire p5_b
);

    localparam [1:0] EVENT_BUTTON_HIGH = 2'd0;
    localparam [1:0] EVENT_BUTTON_LOW  = 2'd1;
    localparam [1:0] EVENT_TICK_12MHZ  = 2'd2;

    wire button_level = BUTTON_ACTIVE_LOW ? ~button_raw : button_raw;

    reg button_meta;
    reg button_sync;
    reg button_sync_d;

    always @(posedge clk_12mhz) begin
        if (rst) begin
            button_meta <= 1'b0;
            button_sync <= 1'b0;
            button_sync_d <= 1'b0;
        end else begin
            button_meta <= button_level;
            button_sync <= button_meta;
            button_sync_d <= button_sync;
        end
    end

    wire button_rise = button_sync & ~button_sync_d;
    wire button_fall = ~button_sync & button_sync_d;

    reg [1:0] event_code;
    always @(*) begin
        if (button_rise) begin
            event_code = EVENT_BUTTON_HIGH;
        end else if (button_fall) begin
            event_code = EVENT_BUTTON_LOW;
        end else begin
            event_code = EVENT_TICK_12MHZ;
        end
    end

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

    SctdPhilosophersLedTop_fsm dut (
        .clk(clk_12mhz),
        .rst(rst),
        .event_valid(1'b1),
        .event_code(event_code),
        .state_out(state_out),
        .transition_taken(transition_taken),
        .dm_timer_count(dm_timer_count),
        .dm_phase_ms(dm_phase_ms),
        .dm_debounce_ms(dm_debounce_ms),
        .dm_p1_r(dm_p1_r),
        .dm_p1_g(dm_p1_g),
        .dm_p1_b(dm_p1_b),
        .dm_p2_r(dm_p2_r),
        .dm_p2_g(dm_p2_g),
        .dm_p2_b(dm_p2_b),
        .dm_p3_r(dm_p3_r),
        .dm_p3_g(dm_p3_g),
        .dm_p3_b(dm_p3_b),
        .dm_p4_r(dm_p4_r),
        .dm_p4_g(dm_p4_g),
        .dm_p4_b(dm_p4_b),
        .dm_p5_r(dm_p5_r),
        .dm_p5_g(dm_p5_g),
        .dm_p5_b(dm_p5_b)
    );

    function led_drive;
        input signed [31:0] dm_value;
        reg logical_on;
        begin
            logical_on = (dm_value != 32'sd0);
            led_drive = LED_ACTIVE_LOW ? ~logical_on : logical_on;
        end
    endfunction

    assign p1_r = led_drive(dm_p1_r);
    assign p1_g = led_drive(dm_p1_g);
    assign p1_b = led_drive(dm_p1_b);
    assign p2_r = led_drive(dm_p2_r);
    assign p2_g = led_drive(dm_p2_g);
    assign p2_b = led_drive(dm_p2_b);
    assign p3_r = led_drive(dm_p3_r);
    assign p3_g = led_drive(dm_p3_g);
    assign p3_b = led_drive(dm_p3_b);
    assign p4_r = led_drive(dm_p4_r);
    assign p4_g = led_drive(dm_p4_g);
    assign p4_b = led_drive(dm_p4_b);
    assign p5_r = led_drive(dm_p5_r);
    assign p5_g = led_drive(dm_p5_g);
    assign p5_b = led_drive(dm_p5_b);

endmodule
