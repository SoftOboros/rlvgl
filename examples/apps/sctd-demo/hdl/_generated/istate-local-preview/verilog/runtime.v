// Generated iState Verilog runtime
// Synchronous state machine implementation

module SctdPhilosophersLedTop_fsm (
    input  wire clk,
    input  wire rst,
    // Event input
    input  wire event_valid,
    input  wire [1:0] event_code,
    // State output
    output reg [3:0] state_out,
    // Transition occurred this cycle
    output reg transition_taken
    // Datamodel: timer_count (16.16 fixed-point)
    ,output reg signed [31:0] dm_timer_count
    // Datamodel: phase_ms (16.16 fixed-point)
    ,output reg signed [31:0] dm_phase_ms
    // Datamodel: debounce_ms (16.16 fixed-point)
    ,output reg signed [31:0] dm_debounce_ms
    // Datamodel: p1_r (16.16 fixed-point)
    ,output reg signed [31:0] dm_p1_r
    // Datamodel: p1_g (16.16 fixed-point)
    ,output reg signed [31:0] dm_p1_g
    // Datamodel: p1_b (16.16 fixed-point)
    ,output reg signed [31:0] dm_p1_b
    // Datamodel: p2_r (16.16 fixed-point)
    ,output reg signed [31:0] dm_p2_r
    // Datamodel: p2_g (16.16 fixed-point)
    ,output reg signed [31:0] dm_p2_g
    // Datamodel: p2_b (16.16 fixed-point)
    ,output reg signed [31:0] dm_p2_b
    // Datamodel: p3_r (16.16 fixed-point)
    ,output reg signed [31:0] dm_p3_r
    // Datamodel: p3_g (16.16 fixed-point)
    ,output reg signed [31:0] dm_p3_g
    // Datamodel: p3_b (16.16 fixed-point)
    ,output reg signed [31:0] dm_p3_b
    // Datamodel: p4_r (16.16 fixed-point)
    ,output reg signed [31:0] dm_p4_r
    // Datamodel: p4_g (16.16 fixed-point)
    ,output reg signed [31:0] dm_p4_g
    // Datamodel: p4_b (16.16 fixed-point)
    ,output reg signed [31:0] dm_p4_b
    // Datamodel: p5_r (16.16 fixed-point)
    ,output reg signed [31:0] dm_p5_r
    // Datamodel: p5_g (16.16 fixed-point)
    ,output reg signed [31:0] dm_p5_g
    // Datamodel: p5_b (16.16 fixed-point)
    ,output reg signed [31:0] dm_p5_b
);

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

// Internal state
reg [3:0] current_state;
reg [3:0] next_state;
reg trans_taken_comb;

reg signed [31:0] dm_reg_timer_count;
reg signed [31:0] dm_next_timer_count;
reg signed [31:0] dm_reg_phase_ms;
reg signed [31:0] dm_next_phase_ms;
reg signed [31:0] dm_reg_debounce_ms;
reg signed [31:0] dm_next_debounce_ms;
reg signed [31:0] dm_reg_p1_r;
reg signed [31:0] dm_next_p1_r;
reg signed [31:0] dm_reg_p1_g;
reg signed [31:0] dm_next_p1_g;
reg signed [31:0] dm_reg_p1_b;
reg signed [31:0] dm_next_p1_b;
reg signed [31:0] dm_reg_p2_r;
reg signed [31:0] dm_next_p2_r;
reg signed [31:0] dm_reg_p2_g;
reg signed [31:0] dm_next_p2_g;
reg signed [31:0] dm_reg_p2_b;
reg signed [31:0] dm_next_p2_b;
reg signed [31:0] dm_reg_p3_r;
reg signed [31:0] dm_next_p3_r;
reg signed [31:0] dm_reg_p3_g;
reg signed [31:0] dm_next_p3_g;
reg signed [31:0] dm_reg_p3_b;
reg signed [31:0] dm_next_p3_b;
reg signed [31:0] dm_reg_p4_r;
reg signed [31:0] dm_next_p4_r;
reg signed [31:0] dm_reg_p4_g;
reg signed [31:0] dm_next_p4_g;
reg signed [31:0] dm_reg_p4_b;
reg signed [31:0] dm_next_p4_b;
reg signed [31:0] dm_reg_p5_r;
reg signed [31:0] dm_next_p5_r;
reg signed [31:0] dm_reg_p5_g;
reg signed [31:0] dm_next_p5_g;
reg signed [31:0] dm_reg_p5_b;
reg signed [31:0] dm_next_p5_b;


// Guard signals
wire guard_t2;
assign guard_t2 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t3;
assign guard_t3 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_debounce_ms < 32'sd589824));
wire guard_t4;
assign guard_t4 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_debounce_ms >= 32'sd589824));
wire guard_t6;
assign guard_t6 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t7;
assign guard_t7 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_debounce_ms < 32'sd589824));
wire guard_t8;
assign guard_t8 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_debounce_ms >= 32'sd589824));
wire guard_t10;
assign guard_t10 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t11;
assign guard_t11 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t12;
assign guard_t12 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t14;
assign guard_t14 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t15;
assign guard_t15 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t16;
assign guard_t16 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t18;
assign guard_t18 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t19;
assign guard_t19 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t20;
assign guard_t20 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t22;
assign guard_t22 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t23;
assign guard_t23 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t24;
assign guard_t24 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t26;
assign guard_t26 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t27;
assign guard_t27 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t28;
assign guard_t28 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t30;
assign guard_t30 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t31;
assign guard_t31 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t32;
assign guard_t32 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));
wire guard_t34;
assign guard_t34 = (dm_reg_timer_count < 32'sd786366464);
wire guard_t35;
assign guard_t35 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms < 32'sd32702464));
wire guard_t36;
assign guard_t36 = ((dm_reg_timer_count >= 32'sd786366464) && (dm_reg_phase_ms >= 32'sd32702464));

// Transition logic (combinational)
always @(*) begin
    next_state = current_state;
    trans_taken_comb = 1'b0;
    dm_next_timer_count = dm_reg_timer_count;
    dm_next_phase_ms = dm_reg_phase_ms;
    dm_next_debounce_ms = dm_reg_debounce_ms;
    dm_next_p1_r = dm_reg_p1_r;
    dm_next_p1_g = dm_reg_p1_g;
    dm_next_p1_b = dm_reg_p1_b;
    dm_next_p2_r = dm_reg_p2_r;
    dm_next_p2_g = dm_reg_p2_g;
    dm_next_p2_b = dm_reg_p2_b;
    dm_next_p3_r = dm_reg_p3_r;
    dm_next_p3_g = dm_reg_p3_g;
    dm_next_p3_b = dm_reg_p3_b;
    dm_next_p4_r = dm_reg_p4_r;
    dm_next_p4_g = dm_reg_p4_g;
    dm_next_p4_b = dm_reg_p4_b;
    dm_next_p5_r = dm_reg_p5_r;
    dm_next_p5_g = dm_reg_p5_g;
    dm_next_p5_b = dm_reg_p5_b;

    if (event_valid) begin
        case ({current_state, event_code})
            {STATE_IDLERELEASED, EVENT_BUTTON_HIGH}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCEPRESS;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                end
            end
            {STATE_DEBOUNCEPRESS, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_IDLERELEASED;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_DEBOUNCEPRESS, EVENT_TICK_12MHZ}: begin
if (guard_t2) begin
                    next_state = STATE_DEBOUNCEPRESS;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t3) begin
                    next_state = STATE_DEBOUNCEPRESS;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_debounce_ms = dm_reg_debounce_ms + 32'sd65536;
                end
else if (guard_t4) begin
                    next_state = STATE_THINKALL;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd0;
                    dm_next_p1_g = 32'sd0;
                    dm_next_p1_b = 32'sd65536;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd65536;
                    dm_next_p3_r = 32'sd0;
                    dm_next_p3_g = 32'sd0;
                    dm_next_p3_b = 32'sd65536;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd65536;
                    dm_next_p5_r = 32'sd0;
                    dm_next_p5_g = 32'sd0;
                    dm_next_p5_b = 32'sd65536;
                end
            end
            {STATE_DEBOUNCERELEASE, EVENT_BUTTON_HIGH}: begin
if (1'b1) begin
                    next_state = STATE_THINKALL;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_DEBOUNCERELEASE, EVENT_TICK_12MHZ}: begin
if (guard_t6) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t7) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_debounce_ms = dm_reg_debounce_ms + 32'sd65536;
                end
else if (guard_t8) begin
                    next_state = STATE_IDLERELEASED;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd0;
                    dm_next_p1_g = 32'sd0;
                    dm_next_p1_b = 32'sd65536;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd65536;
                    dm_next_p3_r = 32'sd0;
                    dm_next_p3_g = 32'sd0;
                    dm_next_p3_b = 32'sd65536;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd65536;
                    dm_next_p5_r = 32'sd0;
                    dm_next_p5_g = 32'sd0;
                    dm_next_p5_b = 32'sd65536;
                end
            end
            {STATE_THINKALL, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_THINKALL, EVENT_TICK_12MHZ}: begin
if (guard_t10) begin
                    next_state = STATE_THINKALL;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t11) begin
                    next_state = STATE_THINKALL;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t12) begin
                    next_state = STATE_HUNGRYODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd65536;
                    dm_next_p1_g = 32'sd0;
                    dm_next_p1_b = 32'sd0;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd65536;
                    dm_next_p3_r = 32'sd65536;
                    dm_next_p3_g = 32'sd0;
                    dm_next_p3_b = 32'sd0;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd65536;
                    dm_next_p5_r = 32'sd0;
                    dm_next_p5_g = 32'sd0;
                    dm_next_p5_b = 32'sd65536;
                end
            end
            {STATE_HUNGRYODD, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_HUNGRYODD, EVENT_TICK_12MHZ}: begin
if (guard_t14) begin
                    next_state = STATE_HUNGRYODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t15) begin
                    next_state = STATE_HUNGRYODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t16) begin
                    next_state = STATE_EATODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd0;
                    dm_next_p1_g = 32'sd65536;
                    dm_next_p1_b = 32'sd0;
                    dm_next_p3_r = 32'sd0;
                    dm_next_p3_g = 32'sd65536;
                    dm_next_p3_b = 32'sd0;
                end
            end
            {STATE_EATODD, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_EATODD, EVENT_TICK_12MHZ}: begin
if (guard_t18) begin
                    next_state = STATE_EATODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t19) begin
                    next_state = STATE_EATODD;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t20) begin
                    next_state = STATE_HUNGRYEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd0;
                    dm_next_p1_g = 32'sd0;
                    dm_next_p1_b = 32'sd65536;
                    dm_next_p2_r = 32'sd65536;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd0;
                    dm_next_p3_r = 32'sd0;
                    dm_next_p3_g = 32'sd0;
                    dm_next_p3_b = 32'sd65536;
                    dm_next_p4_r = 32'sd65536;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd0;
                end
            end
            {STATE_HUNGRYEVEN, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_HUNGRYEVEN, EVENT_TICK_12MHZ}: begin
if (guard_t22) begin
                    next_state = STATE_HUNGRYEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t23) begin
                    next_state = STATE_HUNGRYEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t24) begin
                    next_state = STATE_EATEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd65536;
                    dm_next_p2_b = 32'sd0;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd65536;
                    dm_next_p4_b = 32'sd0;
                end
            end
            {STATE_EATEVEN, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_EATEVEN, EVENT_TICK_12MHZ}: begin
if (guard_t26) begin
                    next_state = STATE_EATEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t27) begin
                    next_state = STATE_EATEVEN;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t28) begin
                    next_state = STATE_HUNGRYFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd65536;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd65536;
                    dm_next_p5_r = 32'sd65536;
                    dm_next_p5_g = 32'sd0;
                    dm_next_p5_b = 32'sd0;
                end
            end
            {STATE_HUNGRYFIVE, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_HUNGRYFIVE, EVENT_TICK_12MHZ}: begin
if (guard_t30) begin
                    next_state = STATE_HUNGRYFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t31) begin
                    next_state = STATE_HUNGRYFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t32) begin
                    next_state = STATE_EATFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p5_r = 32'sd0;
                    dm_next_p5_g = 32'sd65536;
                    dm_next_p5_b = 32'sd0;
                end
            end
            {STATE_EATFIVE, EVENT_BUTTON_LOW}: begin
if (1'b1) begin
                    next_state = STATE_DEBOUNCERELEASE;
                    trans_taken_comb = 1'b1;
                    dm_next_debounce_ms = 32'sd0;
                    dm_next_timer_count = 32'sd0;
                end
            end
            {STATE_EATFIVE, EVENT_TICK_12MHZ}: begin
if (guard_t34) begin
                    next_state = STATE_EATFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = dm_reg_timer_count + 32'sd65536;
                end
else if (guard_t35) begin
                    next_state = STATE_EATFIVE;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = dm_reg_phase_ms + 32'sd65536;
                end
else if (guard_t36) begin
                    next_state = STATE_THINKALL;
                    trans_taken_comb = 1'b1;
                    dm_next_timer_count = 32'sd0;
                    dm_next_phase_ms = 32'sd0;
                    dm_next_p1_r = 32'sd0;
                    dm_next_p1_g = 32'sd0;
                    dm_next_p1_b = 32'sd65536;
                    dm_next_p2_r = 32'sd0;
                    dm_next_p2_g = 32'sd0;
                    dm_next_p2_b = 32'sd65536;
                    dm_next_p3_r = 32'sd0;
                    dm_next_p3_g = 32'sd0;
                    dm_next_p3_b = 32'sd65536;
                    dm_next_p4_r = 32'sd0;
                    dm_next_p4_g = 32'sd0;
                    dm_next_p4_b = 32'sd65536;
                    dm_next_p5_r = 32'sd0;
                    dm_next_p5_g = 32'sd0;
                    dm_next_p5_b = 32'sd65536;
                end
            end
            default: ; // No transition
        endcase
    end
end

// State register (sequential)
always @(posedge clk) begin
    if (rst) begin
        current_state <= STATE_IDLERELEASED;
        dm_reg_timer_count <= 32'sd0;
        dm_reg_phase_ms <= 32'sd0;
        dm_reg_debounce_ms <= 32'sd0;
        dm_reg_p1_r <= 32'sd0;
        dm_reg_p1_g <= 32'sd0;
        dm_reg_p1_b <= 32'sd65536;
        dm_reg_p2_r <= 32'sd0;
        dm_reg_p2_g <= 32'sd0;
        dm_reg_p2_b <= 32'sd65536;
        dm_reg_p3_r <= 32'sd0;
        dm_reg_p3_g <= 32'sd0;
        dm_reg_p3_b <= 32'sd65536;
        dm_reg_p4_r <= 32'sd0;
        dm_reg_p4_g <= 32'sd0;
        dm_reg_p4_b <= 32'sd65536;
        dm_reg_p5_r <= 32'sd0;
        dm_reg_p5_g <= 32'sd0;
        dm_reg_p5_b <= 32'sd65536;
        transition_taken <= 1'b0;
    end else begin
        current_state <= next_state;
        dm_reg_timer_count <= dm_next_timer_count;
        dm_reg_phase_ms <= dm_next_phase_ms;
        dm_reg_debounce_ms <= dm_next_debounce_ms;
        dm_reg_p1_r <= dm_next_p1_r;
        dm_reg_p1_g <= dm_next_p1_g;
        dm_reg_p1_b <= dm_next_p1_b;
        dm_reg_p2_r <= dm_next_p2_r;
        dm_reg_p2_g <= dm_next_p2_g;
        dm_reg_p2_b <= dm_next_p2_b;
        dm_reg_p3_r <= dm_next_p3_r;
        dm_reg_p3_g <= dm_next_p3_g;
        dm_reg_p3_b <= dm_next_p3_b;
        dm_reg_p4_r <= dm_next_p4_r;
        dm_reg_p4_g <= dm_next_p4_g;
        dm_reg_p4_b <= dm_next_p4_b;
        dm_reg_p5_r <= dm_next_p5_r;
        dm_reg_p5_g <= dm_next_p5_g;
        dm_reg_p5_b <= dm_next_p5_b;
        transition_taken <= trans_taken_comb;
    end
end

// Output assignments
always @(*) begin
    state_out = current_state;
    dm_timer_count = dm_reg_timer_count;
    dm_phase_ms = dm_reg_phase_ms;
    dm_debounce_ms = dm_reg_debounce_ms;
    dm_p1_r = dm_reg_p1_r;
    dm_p1_g = dm_reg_p1_g;
    dm_p1_b = dm_reg_p1_b;
    dm_p2_r = dm_reg_p2_r;
    dm_p2_g = dm_reg_p2_g;
    dm_p2_b = dm_reg_p2_b;
    dm_p3_r = dm_reg_p3_r;
    dm_p3_g = dm_reg_p3_g;
    dm_p3_b = dm_reg_p3_b;
    dm_p4_r = dm_reg_p4_r;
    dm_p4_g = dm_reg_p4_g;
    dm_p4_b = dm_reg_p4_b;
    dm_p5_r = dm_reg_p5_r;
    dm_p5_g = dm_reg_p5_g;
    dm_p5_b = dm_reg_p5_b;
end

endmodule