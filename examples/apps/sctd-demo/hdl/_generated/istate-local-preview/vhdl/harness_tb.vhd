-- Generated iState VHDL testbench
-- Reads events from vectors/events.txt and outputs trace
library IEEE;
use IEEE.STD_LOGIC_1164.ALL;
use IEEE.NUMERIC_STD.ALL;
use STD.TEXTIO.ALL;
use IEEE.STD_LOGIC_TEXTIO.ALL;

entity SctdPhilosophersLedTop_tb is
end entity SctdPhilosophersLedTop_tb;

architecture sim of SctdPhilosophersLedTop_tb is


    -- Clock period
    constant CLK_PERIOD : time := 10 ns;

    -- DUT signals
    signal clk         : std_logic := '0';
    signal rst         : std_logic := '1';
    signal event_valid : std_logic := '0';
    signal event_code  : std_logic_vector(1 downto 0) := (others => '0');
    signal state_out   : std_logic_vector(3 downto 0);
    signal transition_taken : std_logic;
    signal dm_timer_count : signed(31 downto 0);
    signal dm_phase_ms : signed(31 downto 0);
    signal dm_debounce_ms : signed(31 downto 0);
    signal dm_p1_r : signed(31 downto 0);
    signal dm_p1_g : signed(31 downto 0);
    signal dm_p1_b : signed(31 downto 0);
    signal dm_p2_r : signed(31 downto 0);
    signal dm_p2_g : signed(31 downto 0);
    signal dm_p2_b : signed(31 downto 0);
    signal dm_p3_r : signed(31 downto 0);
    signal dm_p3_g : signed(31 downto 0);
    signal dm_p3_b : signed(31 downto 0);
    signal dm_p4_r : signed(31 downto 0);
    signal dm_p4_g : signed(31 downto 0);
    signal dm_p4_b : signed(31 downto 0);
    signal dm_p5_r : signed(31 downto 0);
    signal dm_p5_g : signed(31 downto 0);
    signal dm_p5_b : signed(31 downto 0);

    -- State encoding (for output)
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

    -- Helper function to convert state to string
    function state_to_string(s : std_logic_vector(3 downto 0)) return string is
    begin
        if s = STATE_IDLERELEASED then return "IdleReleased"; end if;
        if s = STATE_DEBOUNCEPRESS then return "DebouncePress"; end if;
        if s = STATE_DEBOUNCERELEASE then return "DebounceRelease"; end if;
        if s = STATE_THINKALL then return "ThinkAll"; end if;
        if s = STATE_HUNGRYODD then return "HungryOdd"; end if;
        if s = STATE_EATODD then return "EatOdd"; end if;
        if s = STATE_HUNGRYEVEN then return "HungryEven"; end if;
        if s = STATE_EATEVEN then return "EatEven"; end if;
        if s = STATE_HUNGRYFIVE then return "HungryFive"; end if;
        if s = STATE_EATFIVE then return "EatFive"; end if;
        return "(unknown)";
    end function;

    -- Helper function to convert event string to code
    function event_from_string(s : string) return std_logic_vector is
    begin
        if s = "button_high" then return EVENT_BUTTON_HIGH; end if;
        if s = "button_low" then return EVENT_BUTTON_LOW; end if;
        if s = "tick_12mhz" then return EVENT_TICK_12MHZ; end if;
        return (others => '1'); -- Invalid
    end function;

    -- Simulation control
    signal sim_done : boolean := false;

begin

    -- DUT instantiation
    dut: entity work.SctdPhilosophersLedTop_fsm
        port map (
            clk => clk,
            rst => rst,
            event_valid => event_valid,
            event_code => event_code,
            state_out => state_out,
            transition_taken => transition_taken
            ,dm_timer_count => dm_timer_count
            ,dm_phase_ms => dm_phase_ms
            ,dm_debounce_ms => dm_debounce_ms
            ,dm_p1_r => dm_p1_r
            ,dm_p1_g => dm_p1_g
            ,dm_p1_b => dm_p1_b
            ,dm_p2_r => dm_p2_r
            ,dm_p2_g => dm_p2_g
            ,dm_p2_b => dm_p2_b
            ,dm_p3_r => dm_p3_r
            ,dm_p3_g => dm_p3_g
            ,dm_p3_b => dm_p3_b
            ,dm_p4_r => dm_p4_r
            ,dm_p4_g => dm_p4_g
            ,dm_p4_b => dm_p4_b
            ,dm_p5_r => dm_p5_r
            ,dm_p5_g => dm_p5_g
            ,dm_p5_b => dm_p5_b
        );

    -- Clock generation
    clk_process: process
    begin
        while not sim_done loop
            clk <= '0';
            wait for CLK_PERIOD / 2;
            clk <= '1';
            wait for CLK_PERIOD / 2;
        end loop;
        wait;
    end process;

    -- Stimulus process
    stim_process: process
        file events_file : text;
        variable line_buf : line;
        variable event_str : string(1 to 64);
        variable str_len : integer;
        variable char : character;
        variable good : boolean;
        variable prev_state : std_logic_vector(3 downto 0);
        file output_file : text;
        variable out_line : line;
    begin
        -- Open output file for trace
        file_open(output_file, "output.trace.txt", write_mode);

        -- Reset sequence
        rst <= '1';
        wait for CLK_PERIOD * 2;
        rst <= '0';
        wait for CLK_PERIOD;

        -- Output initial state entry
        write(out_line, string'("on_entry:"));
        write(out_line, state_to_string(state_out));
        writeline(output_file, out_line);
        report "on_entry:" & state_to_string(state_out);

        -- Open and read events file
        file_open(events_file, "vectors/events.txt", read_mode);

        while not endfile(events_file) loop
            readline(events_file, line_buf);

            -- Read event name from line
            str_len := 0;
            for i in 1 to 64 loop
                if line_buf'length >= i then
                    read(line_buf, char, good);
                    if good and char /= ' ' and char /= CR and char /= LF then
                        str_len := str_len + 1;
                        event_str(str_len) := char;
                    else
                        exit;
                    end if;
                else
                    exit;
                end if;
            end loop;

            if str_len > 0 then
                prev_state := state_out;

                -- Apply event
                event_code <= event_from_string(event_str(1 to str_len));
                event_valid <= '1';
                wait for CLK_PERIOD;
                event_valid <= '0';

                -- Check if transition occurred
                if transition_taken = '1' then
                    -- on_exit
                    write(out_line, string'("on_exit:"));
                    write(out_line, state_to_string(prev_state));
                    writeline(output_file, out_line);
                    report "on_exit:" & state_to_string(prev_state);

                    -- transition
                    write(out_line, string'("transition:"));
                    write(out_line, state_to_string(prev_state));
                    write(out_line, string'("->"));
                    write(out_line, state_to_string(state_out));
                    writeline(output_file, out_line);
                    report "transition:" & state_to_string(prev_state) & "->" & state_to_string(state_out);

                    -- on_entry
                    write(out_line, string'("on_entry:"));
                    write(out_line, state_to_string(state_out));
                    writeline(output_file, out_line);
                    report "on_entry:" & state_to_string(state_out);
                else
                    -- no_transition
                    write(out_line, string'("no_transition:"));
                    write(out_line, state_to_string(prev_state));
                    write(out_line, string'(" on "));
                    write(out_line, event_str(1 to str_len));
                    writeline(output_file, out_line);
                    report "no_transition:" & state_to_string(prev_state) & " on " & event_str(1 to str_len);
                end if;

                wait for CLK_PERIOD;
            end if;
        end loop;

        file_close(events_file);
        file_close(output_file);

        report "Simulation complete. Check output.trace.txt";
        sim_done <= true;
        wait;
    end process;

end architecture sim;