use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

use menu_gen::*;

fn event_from_str(s: &str) -> Option<Event> {
    match s {
        "open_menu" => Some(Event::OpenMenu),
        "play" => Some(Event::Play),
        "back" => Some(Event::Back),
        "open_settings" => Some(Event::OpenSettings),
        "stop" => Some(Event::Stop),
        _ => None,
    }
}

fn state_to_str(s: State) -> &'static str {
    match s {
        State::Idle => "idle",
        State::Menu => "menu",
        State::Settings => "settings",
        State::Playing => "playing",
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let reader: Box<dyn BufRead> = if args.len() > 1 {
        Box::new(BufReader::new(File::open(&args[1])?))
    } else {
        Box::new(BufReader::new(io::stdin()))
    };

    let ie = std::env::var("ISTATE_INTERNAL_EVENTS").ok().map(|v| matches!(v.as_str(), "1"|"true"|"TRUE"|"yes"|"YES" )).unwrap_or(false);
    let log = std::env::var("ISTATE_LOG_TO_STDERR").ok().map(|v| matches!(v.as_str(), "1"|"true"|"TRUE"|"yes"|"YES" ))
        .unwrap_or(true);
    let mut m = Machine::with_options(ie, log);
    let mut out: Vec<String> = Vec::new();
    out.push(format!("on_entry:{}", state_to_str(m.state)));
    println!("{}", out.last().unwrap());
    for line in reader.lines() {
        let line = line?;
        let s = line.trim();
        if s.is_empty() { continue; }
        if let Some(ev) = event_from_str(s) {
            let before = m.state;
            if m.dispatch(ev) {
                out.push(format!("on_exit:{}", state_to_str(before))); println!("{}", out.last().unwrap());
                out.push(format!("transition:{}->{}", state_to_str(before), state_to_str(m.state))); println!("{}", out.last().unwrap());
                out.push(format!("on_entry:{}", state_to_str(m.state))); println!("{}", out.last().unwrap());
            } else {
                out.push(format!("no_transition:{} on {}", state_to_str(before), s)); println!("{}", out.last().unwrap());
            }
        } else {
            eprintln!("unknown event: {}", s);
        }
    }
    if args.len() > 2 {
        let mut golden = String::new();
        BufReader::new(File::open(&args[2])?).read_to_string(&mut golden)?;
        let mut idx = 0usize;
        for line in golden.lines() {
            let got = out.get(idx).map(|s| s.as_str()).unwrap_or("");
            if got != line {
                eprintln!("mismatch at line {}\nexpected: {}\n     got: {}", idx+1, line, got);
                std::process::exit(3);
            }
            idx += 1;
        }
        if idx != out.len() { eprintln!("extra lines produced: {} > {}", out.len(), idx); std::process::exit(4); }
        println!("PASS");
    }
    Ok(())
}