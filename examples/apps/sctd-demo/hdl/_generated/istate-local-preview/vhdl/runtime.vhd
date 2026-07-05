-- Generated iState VHDL runtime
-- Synchronous state machine implementation
library IEEE;
use IEEE.STD_LOGIC_1164.ALL;
use IEEE.NUMERIC_STD.ALL;

entity SctdPhilosophersLedTop_fsm is
    port (
        clk         : in  std_logic;
        rst         : in  std_logic;
        -- Event input (directly encoded)
        event_valid : in  std_logic;
        event_code  : in  std_logic_vector(1 downto 0);
        -- State output
        state_out   : out std_logic_vector(3 downto 0);
        -- Transition occurred this cycle
        transition_taken : out std_logic
        -- Datamodel outputs (directly observable)
        ;dm_timer_count : out signed(31 downto 0)
        ;dm_phase_ms : out signed(31 downto 0)
        ;dm_debounce_ms : out signed(31 downto 0)
        ;dm_p1_r : out signed(31 downto 0)
        ;dm_p1_g : out signed(31 downto 0)
        ;dm_p1_b : out signed(31 downto 0)
        ;dm_p2_r : out signed(31 downto 0)
        ;dm_p2_g : out signed(31 downto 0)
        ;dm_p2_b : out signed(31 downto 0)
        ;dm_p3_r : out signed(31 downto 0)
        ;dm_p3_g : out signed(31 downto 0)
        ;dm_p3_b : out signed(31 downto 0)
        ;dm_p4_r : out signed(31 downto 0)
        ;dm_p4_g : out signed(31 downto 0)
        ;dm_p4_b : out signed(31 downto 0)
        ;dm_p5_r : out signed(31 downto 0)
        ;dm_p5_g : out signed(31 downto 0)
        ;dm_p5_b : out signed(31 downto 0)
    );
end entity SctdPhilosophersLedTop_fsm;

architecture behavioral of SctdPhilosophersLedTop_fsm is

    -- State encoding
    constant STATE_IDLERELEASED : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(0, 4));
    constant STATE_DEBOUNCEPRESS : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(1, 4));
    constant STATE_DEBOUNCERELEASE : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(2, 4));
    constant STATE_THINKALL : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(3, 4));
    constant STATE_HUNGRYODD : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(4, 4));
    constant STATE_EATODD : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(5, 4));
    constant STATE_HUNGRYEVEN : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(6, 4));
    constant STATE_EATEVEN : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(7, 4));
    constant STATE_HUNGRYFIVE : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(8, 4));
    constant STATE_EATFIVE : std_logic_vector(3 downto 0) := std_logic_vector(to_unsigned(9, 4));

    -- Event encoding
    constant EVENT_BUTTON_HIGH : std_logic_vector(1 downto 0) := std_logic_vector(to_unsigned(0, 2));
    constant EVENT_BUTTON_LOW : std_logic_vector(1 downto 0) := std_logic_vector(to_unsigned(1, 2));
    constant EVENT_TICK_12MHZ : std_logic_vector(1 downto 0) := std_logic_vector(to_unsigned(2, 2));

    -- State register
    signal current_state : std_logic_vector(3 downto 0);
    signal next_state    : std_logic_vector(3 downto 0);
    signal trans_taken   : std_logic;

    -- Datamodel registers (using fixed-point: 16.16 format scaled to integer for simplicity)
    signal dm_reg_timer_count : signed(31 downto 0);
    signal dm_next_timer_count : signed(31 downto 0);
    signal dm_reg_phase_ms : signed(31 downto 0);
    signal dm_next_phase_ms : signed(31 downto 0);
    signal dm_reg_debounce_ms : signed(31 downto 0);
    signal dm_next_debounce_ms : signed(31 downto 0);
    signal dm_reg_p1_r : signed(31 downto 0);
    signal dm_next_p1_r : signed(31 downto 0);
    signal dm_reg_p1_g : signed(31 downto 0);
    signal dm_next_p1_g : signed(31 downto 0);
    signal dm_reg_p1_b : signed(31 downto 0);
    signal dm_next_p1_b : signed(31 downto 0);
    signal dm_reg_p2_r : signed(31 downto 0);
    signal dm_next_p2_r : signed(31 downto 0);
    signal dm_reg_p2_g : signed(31 downto 0);
    signal dm_next_p2_g : signed(31 downto 0);
    signal dm_reg_p2_b : signed(31 downto 0);
    signal dm_next_p2_b : signed(31 downto 0);
    signal dm_reg_p3_r : signed(31 downto 0);
    signal dm_next_p3_r : signed(31 downto 0);
    signal dm_reg_p3_g : signed(31 downto 0);
    signal dm_next_p3_g : signed(31 downto 0);
    signal dm_reg_p3_b : signed(31 downto 0);
    signal dm_next_p3_b : signed(31 downto 0);
    signal dm_reg_p4_r : signed(31 downto 0);
    signal dm_next_p4_r : signed(31 downto 0);
    signal dm_reg_p4_g : signed(31 downto 0);
    signal dm_next_p4_g : signed(31 downto 0);
    signal dm_reg_p4_b : signed(31 downto 0);
    signal dm_next_p4_b : signed(31 downto 0);
    signal dm_reg_p5_r : signed(31 downto 0);
    signal dm_next_p5_r : signed(31 downto 0);
    signal dm_reg_p5_g : signed(31 downto 0);
    signal dm_next_p5_g : signed(31 downto 0);
    signal dm_reg_p5_b : signed(31 downto 0);
    signal dm_next_p5_b : signed(31 downto 0);


    -- Guard signals
    signal guard_t2 : boolean;
    signal guard_t3 : boolean;
    signal guard_t4 : boolean;
    signal guard_t6 : boolean;
    signal guard_t7 : boolean;
    signal guard_t8 : boolean;
    signal guard_t10 : boolean;
    signal guard_t11 : boolean;
    signal guard_t12 : boolean;
    signal guard_t14 : boolean;
    signal guard_t15 : boolean;
    signal guard_t16 : boolean;
    signal guard_t18 : boolean;
    signal guard_t19 : boolean;
    signal guard_t20 : boolean;
    signal guard_t22 : boolean;
    signal guard_t23 : boolean;
    signal guard_t24 : boolean;
    signal guard_t26 : boolean;
    signal guard_t27 : boolean;
    signal guard_t28 : boolean;
    signal guard_t30 : boolean;
    signal guard_t31 : boolean;
    signal guard_t32 : boolean;
    signal guard_t34 : boolean;
    signal guard_t35 : boolean;
    signal guard_t36 : boolean;

begin

    -- Guard logic (combinational)
    guard_t2 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t3 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_debounce_ms < to_signed(589824, 32)));
    guard_t4 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_debounce_ms >= to_signed(589824, 32)));
    guard_t6 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t7 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_debounce_ms < to_signed(589824, 32)));
    guard_t8 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_debounce_ms >= to_signed(589824, 32)));
    guard_t10 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t11 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t12 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t14 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t15 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t16 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t18 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t19 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t20 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t22 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t23 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t24 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t26 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t27 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t28 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t30 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t31 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t32 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));
    guard_t34 <= (dm_reg_timer_count < to_signed(786366464, 32));
    guard_t35 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms < to_signed(32702464, 32)));
    guard_t36 <= ((dm_reg_timer_count >= to_signed(786366464, 32)) and (dm_reg_phase_ms >= to_signed(32702464, 32)));

    -- Transition logic (combinational)
    process(current_state, event_valid, event_code, guard_t2, guard_t3, guard_t4, guard_t6, guard_t7, guard_t8, guard_t10, guard_t11, guard_t12, guard_t14, guard_t15, guard_t16, guard_t18, guard_t19, guard_t20, guard_t22, guard_t23, guard_t24, guard_t26, guard_t27, guard_t28, guard_t30, guard_t31, guard_t32, guard_t34, guard_t35, guard_t36, dm_reg_timer_count, dm_reg_phase_ms, dm_reg_debounce_ms, dm_reg_p1_r, dm_reg_p1_g, dm_reg_p1_b, dm_reg_p2_r, dm_reg_p2_g, dm_reg_p2_b, dm_reg_p3_r, dm_reg_p3_g, dm_reg_p3_b, dm_reg_p4_r, dm_reg_p4_g, dm_reg_p4_b, dm_reg_p5_r, dm_reg_p5_g, dm_reg_p5_b)
    begin
        next_state <= current_state;
        trans_taken <= '0';
        dm_next_timer_count <= dm_reg_timer_count;
        dm_next_phase_ms <= dm_reg_phase_ms;
        dm_next_debounce_ms <= dm_reg_debounce_ms;
        dm_next_p1_r <= dm_reg_p1_r;
        dm_next_p1_g <= dm_reg_p1_g;
        dm_next_p1_b <= dm_reg_p1_b;
        dm_next_p2_r <= dm_reg_p2_r;
        dm_next_p2_g <= dm_reg_p2_g;
        dm_next_p2_b <= dm_reg_p2_b;
        dm_next_p3_r <= dm_reg_p3_r;
        dm_next_p3_g <= dm_reg_p3_g;
        dm_next_p3_b <= dm_reg_p3_b;
        dm_next_p4_r <= dm_reg_p4_r;
        dm_next_p4_g <= dm_reg_p4_g;
        dm_next_p4_b <= dm_reg_p4_b;
        dm_next_p5_r <= dm_reg_p5_r;
        dm_next_p5_g <= dm_reg_p5_g;
        dm_next_p5_b <= dm_reg_p5_b;

        if event_valid = '1' then
if current_state = STATE_IDLERELEASED and event_code = EVENT_BUTTON_HIGH then
                next_state <= STATE_DEBOUNCEPRESS;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
elsif current_state = STATE_DEBOUNCEPRESS and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_IDLERELEASED;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_DEBOUNCEPRESS and event_code = EVENT_TICK_12MHZ and guard_t2 then
                next_state <= STATE_DEBOUNCEPRESS;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_DEBOUNCEPRESS and event_code = EVENT_TICK_12MHZ and guard_t3 then
                next_state <= STATE_DEBOUNCEPRESS;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_debounce_ms <= dm_reg_debounce_ms + to_signed(65536, 32);
elsif current_state = STATE_DEBOUNCEPRESS and event_code = EVENT_TICK_12MHZ and guard_t4 then
                next_state <= STATE_THINKALL;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(0, 32);
                dm_next_p1_g <= to_signed(0, 32);
                dm_next_p1_b <= to_signed(65536, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(65536, 32);
                dm_next_p3_r <= to_signed(0, 32);
                dm_next_p3_g <= to_signed(0, 32);
                dm_next_p3_b <= to_signed(65536, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(65536, 32);
                dm_next_p5_r <= to_signed(0, 32);
                dm_next_p5_g <= to_signed(0, 32);
                dm_next_p5_b <= to_signed(65536, 32);
elsif current_state = STATE_DEBOUNCERELEASE and event_code = EVENT_BUTTON_HIGH then
                next_state <= STATE_THINKALL;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_DEBOUNCERELEASE and event_code = EVENT_TICK_12MHZ and guard_t6 then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_DEBOUNCERELEASE and event_code = EVENT_TICK_12MHZ and guard_t7 then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_debounce_ms <= dm_reg_debounce_ms + to_signed(65536, 32);
elsif current_state = STATE_DEBOUNCERELEASE and event_code = EVENT_TICK_12MHZ and guard_t8 then
                next_state <= STATE_IDLERELEASED;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(0, 32);
                dm_next_p1_g <= to_signed(0, 32);
                dm_next_p1_b <= to_signed(65536, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(65536, 32);
                dm_next_p3_r <= to_signed(0, 32);
                dm_next_p3_g <= to_signed(0, 32);
                dm_next_p3_b <= to_signed(65536, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(65536, 32);
                dm_next_p5_r <= to_signed(0, 32);
                dm_next_p5_g <= to_signed(0, 32);
                dm_next_p5_b <= to_signed(65536, 32);
elsif current_state = STATE_THINKALL and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_THINKALL and event_code = EVENT_TICK_12MHZ and guard_t10 then
                next_state <= STATE_THINKALL;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_THINKALL and event_code = EVENT_TICK_12MHZ and guard_t11 then
                next_state <= STATE_THINKALL;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_THINKALL and event_code = EVENT_TICK_12MHZ and guard_t12 then
                next_state <= STATE_HUNGRYODD;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(65536, 32);
                dm_next_p1_g <= to_signed(0, 32);
                dm_next_p1_b <= to_signed(0, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(65536, 32);
                dm_next_p3_r <= to_signed(65536, 32);
                dm_next_p3_g <= to_signed(0, 32);
                dm_next_p3_b <= to_signed(0, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(65536, 32);
                dm_next_p5_r <= to_signed(0, 32);
                dm_next_p5_g <= to_signed(0, 32);
                dm_next_p5_b <= to_signed(65536, 32);
elsif current_state = STATE_HUNGRYODD and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_HUNGRYODD and event_code = EVENT_TICK_12MHZ and guard_t14 then
                next_state <= STATE_HUNGRYODD;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYODD and event_code = EVENT_TICK_12MHZ and guard_t15 then
                next_state <= STATE_HUNGRYODD;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYODD and event_code = EVENT_TICK_12MHZ and guard_t16 then
                next_state <= STATE_EATODD;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(0, 32);
                dm_next_p1_g <= to_signed(65536, 32);
                dm_next_p1_b <= to_signed(0, 32);
                dm_next_p3_r <= to_signed(0, 32);
                dm_next_p3_g <= to_signed(65536, 32);
                dm_next_p3_b <= to_signed(0, 32);
elsif current_state = STATE_EATODD and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_EATODD and event_code = EVENT_TICK_12MHZ and guard_t18 then
                next_state <= STATE_EATODD;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_EATODD and event_code = EVENT_TICK_12MHZ and guard_t19 then
                next_state <= STATE_EATODD;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_EATODD and event_code = EVENT_TICK_12MHZ and guard_t20 then
                next_state <= STATE_HUNGRYEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(0, 32);
                dm_next_p1_g <= to_signed(0, 32);
                dm_next_p1_b <= to_signed(65536, 32);
                dm_next_p2_r <= to_signed(65536, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(0, 32);
                dm_next_p3_r <= to_signed(0, 32);
                dm_next_p3_g <= to_signed(0, 32);
                dm_next_p3_b <= to_signed(65536, 32);
                dm_next_p4_r <= to_signed(65536, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(0, 32);
elsif current_state = STATE_HUNGRYEVEN and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_HUNGRYEVEN and event_code = EVENT_TICK_12MHZ and guard_t22 then
                next_state <= STATE_HUNGRYEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYEVEN and event_code = EVENT_TICK_12MHZ and guard_t23 then
                next_state <= STATE_HUNGRYEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYEVEN and event_code = EVENT_TICK_12MHZ and guard_t24 then
                next_state <= STATE_EATEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(65536, 32);
                dm_next_p2_b <= to_signed(0, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(65536, 32);
                dm_next_p4_b <= to_signed(0, 32);
elsif current_state = STATE_EATEVEN and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_EATEVEN and event_code = EVENT_TICK_12MHZ and guard_t26 then
                next_state <= STATE_EATEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_EATEVEN and event_code = EVENT_TICK_12MHZ and guard_t27 then
                next_state <= STATE_EATEVEN;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_EATEVEN and event_code = EVENT_TICK_12MHZ and guard_t28 then
                next_state <= STATE_HUNGRYFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(65536, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(65536, 32);
                dm_next_p5_r <= to_signed(65536, 32);
                dm_next_p5_g <= to_signed(0, 32);
                dm_next_p5_b <= to_signed(0, 32);
elsif current_state = STATE_HUNGRYFIVE and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_HUNGRYFIVE and event_code = EVENT_TICK_12MHZ and guard_t30 then
                next_state <= STATE_HUNGRYFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYFIVE and event_code = EVENT_TICK_12MHZ and guard_t31 then
                next_state <= STATE_HUNGRYFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_HUNGRYFIVE and event_code = EVENT_TICK_12MHZ and guard_t32 then
                next_state <= STATE_EATFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p5_r <= to_signed(0, 32);
                dm_next_p5_g <= to_signed(65536, 32);
                dm_next_p5_b <= to_signed(0, 32);
elsif current_state = STATE_EATFIVE and event_code = EVENT_BUTTON_LOW then
                next_state <= STATE_DEBOUNCERELEASE;
                trans_taken <= '1';
                dm_next_debounce_ms <= to_signed(0, 32);
                dm_next_timer_count <= to_signed(0, 32);
elsif current_state = STATE_EATFIVE and event_code = EVENT_TICK_12MHZ and guard_t34 then
                next_state <= STATE_EATFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= dm_reg_timer_count + to_signed(65536, 32);
elsif current_state = STATE_EATFIVE and event_code = EVENT_TICK_12MHZ and guard_t35 then
                next_state <= STATE_EATFIVE;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= dm_reg_phase_ms + to_signed(65536, 32);
elsif current_state = STATE_EATFIVE and event_code = EVENT_TICK_12MHZ and guard_t36 then
                next_state <= STATE_THINKALL;
                trans_taken <= '1';
                dm_next_timer_count <= to_signed(0, 32);
                dm_next_phase_ms <= to_signed(0, 32);
                dm_next_p1_r <= to_signed(0, 32);
                dm_next_p1_g <= to_signed(0, 32);
                dm_next_p1_b <= to_signed(65536, 32);
                dm_next_p2_r <= to_signed(0, 32);
                dm_next_p2_g <= to_signed(0, 32);
                dm_next_p2_b <= to_signed(65536, 32);
                dm_next_p3_r <= to_signed(0, 32);
                dm_next_p3_g <= to_signed(0, 32);
                dm_next_p3_b <= to_signed(65536, 32);
                dm_next_p4_r <= to_signed(0, 32);
                dm_next_p4_g <= to_signed(0, 32);
                dm_next_p4_b <= to_signed(65536, 32);
                dm_next_p5_r <= to_signed(0, 32);
                dm_next_p5_g <= to_signed(0, 32);
                dm_next_p5_b <= to_signed(65536, 32);
            end if;
        end if;
    end process;

    -- State register (sequential)
    process(clk)
    begin
        if rising_edge(clk) then
            if rst = '1' then
                current_state <= STATE_IDLERELEASED;
                dm_reg_timer_count <= to_signed(0, 32);
                dm_reg_phase_ms <= to_signed(0, 32);
                dm_reg_debounce_ms <= to_signed(0, 32);
                dm_reg_p1_r <= to_signed(0, 32);
                dm_reg_p1_g <= to_signed(0, 32);
                dm_reg_p1_b <= to_signed(65536, 32);
                dm_reg_p2_r <= to_signed(0, 32);
                dm_reg_p2_g <= to_signed(0, 32);
                dm_reg_p2_b <= to_signed(65536, 32);
                dm_reg_p3_r <= to_signed(0, 32);
                dm_reg_p3_g <= to_signed(0, 32);
                dm_reg_p3_b <= to_signed(65536, 32);
                dm_reg_p4_r <= to_signed(0, 32);
                dm_reg_p4_g <= to_signed(0, 32);
                dm_reg_p4_b <= to_signed(65536, 32);
                dm_reg_p5_r <= to_signed(0, 32);
                dm_reg_p5_g <= to_signed(0, 32);
                dm_reg_p5_b <= to_signed(65536, 32);
            else
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
            end if;
        end if;
    end process;

    -- Output assignments
    state_out <= current_state;
    transition_taken <= trans_taken;
    dm_timer_count <= dm_reg_timer_count;
    dm_phase_ms <= dm_reg_phase_ms;
    dm_debounce_ms <= dm_reg_debounce_ms;
    dm_p1_r <= dm_reg_p1_r;
    dm_p1_g <= dm_reg_p1_g;
    dm_p1_b <= dm_reg_p1_b;
    dm_p2_r <= dm_reg_p2_r;
    dm_p2_g <= dm_reg_p2_g;
    dm_p2_b <= dm_reg_p2_b;
    dm_p3_r <= dm_reg_p3_r;
    dm_p3_g <= dm_reg_p3_g;
    dm_p3_b <= dm_reg_p3_b;
    dm_p4_r <= dm_reg_p4_r;
    dm_p4_g <= dm_reg_p4_g;
    dm_p4_b <= dm_reg_p4_b;
    dm_p5_r <= dm_reg_p5_r;
    dm_p5_g <= dm_reg_p5_g;
    dm_p5_b <= dm_reg_p5_b;

end architecture behavioral;