//! AutoHotkey scripts in and out of Pious's macros.
//!
//! Importing reads the parts of AutoHotkey (v1 and v2) that macros are made
//! of: `Send` and its relatives with `{Key}`, `{Key down}`, `{Key 3}` and
//! the `^!+#` modifiers, `Sleep`, `Click`, `MouseClick`, `MouseMove`,
//! `Loop`, `WinActivate` (Roblox), `Run`, and a hotkey label like `F1::`
//! (which becomes the macro's hotkey). Anything else is kept as a note in
//! the macro, so nothing is silently lost. Exporting writes an AutoHotkey
//! v2 script that does the same as the macro.

use super::automation::{Button, Macro, Press, Repeat, Step};

/// What an imported script turned into.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub steps: Vec<Step>,
    /// From a hotkey label (`F1::`), in Pious's hotkey format.
    pub hotkey: Option<String>,
    /// Lines that were kept as notes.
    pub skipped: usize,
}

/// AutoHotkey's key names → Pious's.
fn key_code(name: &str) -> Option<String> {
    let n = name.trim();
    let lower = n.to_ascii_lowercase();
    let named = match lower.as_str() {
        "enter" | "return" => "Enter",
        "space" => "Space",
        "tab" => "Tab",
        "esc" | "escape" => "Escape",
        "backspace" | "bs" => "Backspace",
        "delete" | "del" => "Delete",
        "insert" | "ins" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pgup" => "PageUp",
        "pgdn" => "PageDown",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "shift" | "lshift" => "ShiftLeft",
        "rshift" => "ShiftRight",
        "ctrl" | "control" | "lctrl" | "lcontrol" => "ControlLeft",
        "rctrl" | "rcontrol" => "ControlRight",
        "alt" | "lalt" => "AltLeft",
        "ralt" => "AltRight",
        "lwin" | "win" => "MetaLeft",
        "rwin" => "MetaRight",
        "capslock" => "CapsLock",
        "numlock" => "NumLock",
        "scrolllock" => "ScrollLock",
        "printscreen" => "PrintScreen",
        "pause" => "Pause",
        "appskey" => "ContextMenu",
        "numpadenter" => "NumpadEnter",
        "numpadadd" => "NumpadAdd",
        "numpadsub" => "NumpadSubtract",
        "numpadmult" => "NumpadMultiply",
        "numpaddiv" => "NumpadDivide",
        "numpaddot" => "NumpadDecimal",
        "-" => "Minus",
        "=" => "Equal",
        "[" => "BracketLeft",
        "]" => "BracketRight",
        "\\" => "Backslash",
        ";" => "Semicolon",
        "'" => "Quote",
        "`" => "Backquote",
        "," => "Comma",
        "." => "Period",
        "/" => "Slash",
        _ => "",
    };
    if !named.is_empty() {
        return Some(named.into());
    }
    if n.len() == 1 {
        let c = n.chars().next()?;
        if c.is_ascii_alphabetic() {
            return Some(format!("Key{}", c.to_ascii_uppercase()));
        }
        if c.is_ascii_digit() {
            return Some(format!("Digit{c}"));
        }
    }
    if let Some(d) = lower.strip_prefix("numpad").filter(|d| d.len() == 1 && d.chars().all(|c| c.is_ascii_digit())) {
        return Some(format!("Numpad{d}"));
    }
    if let Some(f) = lower.strip_prefix('f').and_then(|f| f.parse::<u8>().ok()).filter(|f| (1..=24).contains(f)) {
        return Some(format!("F{f}"));
    }
    None
}

/// Pious's key names → AutoHotkey's.
fn ahk_key(code: &str) -> String {
    let named = match code {
        "Enter" => "Enter",
        "Space" => "Space",
        "Tab" => "Tab",
        "Escape" => "Esc",
        "Backspace" => "Backspace",
        "Delete" => "Delete",
        "Insert" => "Insert",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PgUp",
        "PageDown" => "PgDn",
        "ArrowUp" => "Up",
        "ArrowDown" => "Down",
        "ArrowLeft" => "Left",
        "ArrowRight" => "Right",
        "ShiftLeft" | "Shift" => "LShift",
        "ShiftRight" => "RShift",
        "ControlLeft" | "Control" => "LCtrl",
        "ControlRight" => "RCtrl",
        "AltLeft" | "Alt" => "LAlt",
        "AltRight" => "RAlt",
        "MetaLeft" | "Meta" | "Super" => "LWin",
        "MetaRight" => "RWin",
        "CapsLock" => "CapsLock",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Backquote" => "`",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        _ => "",
    };
    if !named.is_empty() {
        return named.into();
    }
    if let Some(c) = code.strip_prefix("Key") {
        return c.to_ascii_lowercase();
    }
    if let Some(d) = code.strip_prefix("Digit") {
        return d.into();
    }
    code.into()
}

fn mouse_button(name: &str) -> Option<Button> {
    match name.trim().to_ascii_lowercase().as_str() {
        "l" | "left" | "lbutton" => Some(Button::Left),
        "r" | "right" | "rbutton" => Some(Button::Right),
        "m" | "middle" | "mbutton" => Some(Button::Middle),
        "x1" | "xbutton1" => Some(Button::Back),
        "x2" | "xbutton2" => Some(Button::Forward),
        _ => None,
    }
}

/// A hotkey label's keys (`^!F1`) in Pious's format (`Control+Alt+F1`).
fn hotkey_of(label: &str) -> Option<String> {
    let mut rest = label.trim().trim_start_matches(['~', '*', '$']);
    let mut parts = Vec::new();
    while let Some(c) = rest.chars().next() {
        let modifier = match c {
            '^' => "Control",
            '!' => "Alt",
            '+' => "Shift",
            '#' => "Super",
            _ => break,
        };
        parts.push(modifier.to_owned());
        rest = &rest[1..];
    }
    let key = rest.split_whitespace().next()?;
    parts.push(key_code(key)?);
    Some(parts.join("+"))
}

/// The arguments after a command, v1 (`Sleep, 100`) or v2 (`Sleep 100`,
/// `Sleep(100)`) style, split on commas outside quotes.
fn args(rest: &str) -> Vec<String> {
    let rest = rest.trim().trim_start_matches(',').trim();
    let rest = rest.strip_prefix('(').and_then(|r| r.strip_suffix(')')).unwrap_or(rest);
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in rest.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut current).trim().to_owned()),
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() || !out.is_empty() {
        out.push(current.trim().to_owned());
    }
    out
}

fn number(text: &str) -> Option<i64> {
    text.trim().trim_matches('"').parse::<f64>().ok().map(|n| n.round() as i64)
}

/// `Send` text: `{Key}`, `{Key down}`, `{Key 3}`, modifiers `^!+#` on the
/// next key or `(group)`, and plain characters (typed).
fn send_steps(text: &str, raw: bool) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut plain = String::new();
    let flush = |plain: &mut String, steps: &mut Vec<Step>| {
        if !plain.is_empty() {
            steps.push(Step::Text { text: std::mem::take(plain), delay_ms: 0 });
        }
    };
    let tap = |key: String| Step::Key { key, press: Press::Tap, hold_ms: 30 };
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut held: Vec<String> = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if !raw && matches!(c, '^' | '!' | '+' | '#') {
            flush(&mut plain, &mut steps);
            let modifier = match c {
                '^' => "ControlLeft",
                '!' => "AltLeft",
                '+' => "ShiftLeft",
                _ => "MetaLeft",
            };
            steps.push(Step::Key { key: modifier.into(), press: Press::Down, hold_ms: 0 });
            held.push(modifier.into());
            i += 1;
            continue;
        }
        if !raw && c == '{' {
            if let Some(end) = chars[i + 1..].iter().position(|&x| x == '}').map(|e| e + i + 1) {
                // `{}}` sends a brace.
                let end = if end == i + 1 && chars.get(end + 1) == Some(&'}') { end + 1 } else { end };
                let inner: String = chars[i + 1..end].iter().collect();
                let mut words = inner.split_whitespace();
                let name = words.next().unwrap_or_default();
                let arg = words.next().unwrap_or_default().to_ascii_lowercase();
                match key_code(name) {
                    Some(key) => {
                        flush(&mut plain, &mut steps);
                        match arg.as_str() {
                            "down" => steps.push(Step::Key { key, press: Press::Down, hold_ms: 0 }),
                            "up" => steps.push(Step::Key { key, press: Press::Up, hold_ms: 0 }),
                            n => {
                                let times = n.parse::<u32>().unwrap_or(1).clamp(1, 500);
                                for _ in 0..times {
                                    steps.push(tap(key.clone()));
                                }
                            }
                        }
                    }
                    // `{!}`, `{#}`, a literal character.
                    None if name.chars().count() == 1 => plain.push_str(name),
                    None => {
                        flush(&mut plain, &mut steps);
                        steps.push(Step::Comment { text: format!("AutoHotkey key Pious doesn't know: {{{inner}}}") });
                    }
                }
                i = end + 1;
                for key in held.drain(..).rev() {
                    steps.push(Step::Key { key, press: Press::Up, hold_ms: 0 });
                }
                continue;
            }
        }
        if held.is_empty() {
            plain.push(c);
        } else {
            // A modifier applies to this one key.
            match key_code(&c.to_string()) {
                Some(key) => steps.push(tap(key)),
                None => steps.push(Step::Text { text: c.to_string(), delay_ms: 0 }),
            }
            for key in held.drain(..).rev() {
                steps.push(Step::Key { key, press: Press::Up, hold_ms: 0 });
            }
        }
        i += 1;
    }
    flush(&mut plain, &mut steps);
    for key in held.drain(..).rev() {
        steps.push(Step::Key { key, press: Press::Up, hold_ms: 0 });
    }
    steps
}

/// `Click` and `MouseClick` arguments: a button, a spot, a count, down/up.
fn click_steps(parts: &[String], mouse_click: bool) -> Vec<Step> {
    let words: Vec<String> = if mouse_click {
        parts.to_vec()
    } else {
        parts.iter().flat_map(|p| p.split_whitespace().map(str::to_owned).collect::<Vec<_>>()).collect()
    };
    let mut button = Button::Left;
    let mut press = Press::Tap;
    let mut numbers = Vec::new();
    let mut relative = false;
    for w in &words {
        let lower = w.to_ascii_lowercase();
        if let Some(b) = mouse_button(&lower) {
            button = b;
        } else if lower == "down" || lower == "d" {
            press = Press::Down;
        } else if lower == "up" || lower == "u" {
            press = Press::Up;
        } else if lower == "rel" || lower == "relative" || lower == "r" && mouse_click {
            relative = true;
        } else if let Some(n) = number(w) {
            numbers.push(n);
        }
    }
    let mut steps = Vec::new();
    if numbers.len() >= 2 {
        steps.push(Step::Move { x: numbers[0] as i32, y: numbers[1] as i32, relative, duration_ms: 0 });
    }
    // One number is a count (`Click 2`); with a spot, it comes third.
    let count = if numbers.len() == 1 { numbers[0] } else { numbers.get(2).copied().unwrap_or(1) };
    if count > 0 {
        steps.push(Step::Click { button, press, count: count.clamp(1, 100) as u32 });
    }
    steps
}

/// Reads an AutoHotkey script.
pub fn import(script: &str) -> Imported {
    let mut hotkey = None;
    let mut skipped = 0;
    // Each open `Loop`: how many times, the steps gathered inside it, and
    // whether it has no braces (so it covers only the next line).
    let mut stack: Vec<(u32, Vec<Step>, bool)> = vec![(0, Vec::new(), false)];
    let mut in_comment = false;

    for raw_line in script.lines() {
        let mut line = raw_line.trim();
        if in_comment {
            if line.starts_with("*/") {
                in_comment = false;
            }
            continue;
        }
        if line.starts_with("/*") {
            in_comment = !line.contains("*/");
            continue;
        }
        // Comments: `;` at the start or after a space.
        if line.starts_with(';') {
            continue;
        }
        if let Some(at) = line.find(" ;") {
            line = line[..at].trim_end();
        }
        if line.is_empty() || line.starts_with('#') && !line.contains("::") {
            continue;
        }

        // A hotkey label, maybe with its action on the same line.
        if let Some((label, action)) = line.split_once("::") {
            if !label.is_empty() && !label.contains(' ') || label.starts_with(['^', '!', '+', '#', '~', '*', '$']) {
                if hotkey.is_none() {
                    hotkey = hotkey_of(label);
                }
                line = action.trim();
                if line.is_empty() || line == "{" {
                    continue;
                }
            }
        }

        let lower = line.to_ascii_lowercase();
        let (command, rest) = match line.find(|c: char| c == ',' || c == ' ' || c == '(' || c == '\t') {
            Some(at) => (lower[..at].to_owned(), &line[at..]),
            None => (lower.clone(), ""),
        };
        let steps = &mut stack.last_mut().expect("a level").1;
        match command.as_str() {
            "}" => {
                if stack.len() > 1 {
                    let (times, inner, _) = stack.pop().expect("a loop");
                    stack.last_mut().expect("a level").1.push(Step::Loop { times, steps: inner });
                }
            }
            // The brace of a loop declared on the line before.
            "{" => {
                if let Some(top) = stack.last_mut() {
                    top.2 = false;
                }
                continue;
            }
            "return" | "exitapp" | "reload" => {}
            "loop" => {
                let braced = rest.trim_end().ends_with('{');
                let parts = args(rest.trim_end().trim_end_matches('{'));
                let times = parts.first().and_then(|p| number(p)).unwrap_or(0).clamp(0, 1_000_000) as u32;
                stack.push((times, Vec::new(), !braced));
                continue;
            }
            "sleep" => {
                let ms = args(rest).first().and_then(|p| number(p)).unwrap_or(0).clamp(0, 86_400_000) as u32;
                steps.push(Step::Wait { ms, random_ms: 0 });
            }
            "send" | "sendinput" | "sendevent" | "sendplay" | "sendraw" | "sendtext" => {
                let text = rest.trim().trim_start_matches(',').trim();
                let text = text.strip_prefix('(').and_then(|t| t.strip_suffix(')')).unwrap_or(text);
                let text = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(text);
                let raw = command == "sendraw" || command == "sendtext" || text.starts_with("{Raw}") || text.starts_with("{Text}");
                let text = text.trim_start_matches("{Raw}").trim_start_matches("{Text}");
                steps.extend(send_steps(&text.replace("``", "`").replace("`n", "\n"), raw));
            }
            "click" => steps.extend(click_steps(&args(rest), false)),
            "mouseclick" => steps.extend(click_steps(&args(rest), true)),
            "mousemove" => {
                let parts = args(rest);
                let x = parts.first().and_then(|p| number(p));
                let y = parts.get(1).and_then(|p| number(p));
                let speed = parts.get(2).and_then(|p| number(p)).unwrap_or(0);
                let relative = parts.get(3).is_some_and(|p| p.trim().eq_ignore_ascii_case("r"));
                if let (Some(x), Some(y)) = (x, y) {
                    steps.push(Step::Move { x: x as i32, y: y as i32, relative, duration_ms: (speed.clamp(0, 100) * 10) as u32 });
                }
            }
            "winactivate" | "winwaitactive" if lower.contains("roblox") => steps.push(Step::FocusRoblox),
            "run" => {
                if let Some(target) = args(rest).first().filter(|t| !t.is_empty()) {
                    steps.push(Step::Run { target: target.trim_matches('"').to_owned() });
                }
            }
            "setkeydelay" | "setmousedelay" | "sendmode" | "setbatchlines" | "coordmode" | "settitlematchmode" | "setdefaultmousespeed" => {}
            _ => {
                skipped += 1;
                steps.push(Step::Comment { text: format!("From AutoHotkey (not run): {line}") });
            }
        }
        // A loop without braces ends after its one line.
        while stack.len() > 1 && stack.last().is_some_and(|top| top.2 && !top.1.is_empty()) {
            let (times, inner, _) = stack.pop().expect("a loop");
            stack.last_mut().expect("a level").1.push(Step::Loop { times, steps: inner });
        }
    }
    while stack.len() > 1 {
        let (times, inner, _) = stack.pop().expect("a loop");
        stack.last_mut().expect("a level").1.push(Step::Loop { times, steps: inner });
    }
    Imported { steps: stack.pop().map(|(_, s, _)| s).unwrap_or_default(), hotkey, skipped }
}

fn ahk_button(button: Button) -> &'static str {
    match button {
        Button::Left => "Left",
        Button::Right => "Right",
        Button::Middle => "Middle",
        Button::Back => "X1",
        Button::Forward => "X2",
    }
}

/// A hotkey in Pious's format (`Control+Alt+F1`) as an AutoHotkey label.
fn ahk_hotkey(hotkey: &str) -> Option<String> {
    let mut out = String::new();
    let mut key = None;
    for part in hotkey.split('+') {
        match part {
            "Control" => out.push('^'),
            "Alt" => out.push('!'),
            "Shift" => out.push('+'),
            "Super" | "Meta" => out.push('#'),
            k => key = Some(ahk_key(k)),
        }
    }
    Some(out + &key?)
}

fn write_steps(steps: &[Step], indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    for step in steps {
        let line = match step {
            Step::Key { key, press, hold_ms } => match press {
                Press::Down => format!("Send \"{{{} down}}\"", ahk_key(key)),
                Press::Up => format!("Send \"{{{} up}}\"", ahk_key(key)),
                Press::Tap if *hold_ms > 0 => {
                    format!("Send \"{{{k} down}}\"\n{pad}Sleep {hold_ms}\n{pad}Send \"{{{k} up}}\"", k = ahk_key(key))
                }
                Press::Tap => format!("Send \"{{{}}}\"", ahk_key(key)),
            },
            Step::Text { text, .. } => format!("SendText \"{}\"", text.replace('`', "``").replace('"', "`\"").replace('\n', "`n")),
            Step::Click { button, press, count } => match press {
                Press::Down => format!("Click \"{} Down\"", ahk_button(*button)),
                Press::Up => format!("Click \"{} Up\"", ahk_button(*button)),
                Press::Tap => format!("Click \"{} {}\"", ahk_button(*button), count),
            },
            Step::Move { x, y, relative, duration_ms } => {
                let speed = (duration_ms / 10).min(100);
                if *relative { format!("MouseMove {x}, {y}, {speed}, \"R\"") } else { format!("MouseMove {x}, {y}, {speed}") }
            }
            Step::Scroll { amount, horizontal } => {
                let name = match (horizontal, *amount > 0) {
                    (false, true) => "WheelUp",
                    (false, false) => "WheelDown",
                    (true, true) => "WheelRight",
                    (true, false) => "WheelLeft",
                };
                format!("Click \"{name} {}\"", amount.unsigned_abs())
            }
            Step::Wait { ms, random_ms } => {
                if *random_ms > 0 { format!("Sleep {ms} + Random(0, {random_ms})") } else { format!("Sleep {ms}") }
            }
            Step::Loop { times, steps } => {
                let mut inner = String::new();
                write_steps(steps, indent + 1, &mut inner);
                let head = if *times == 0 { "Loop".to_owned() } else { format!("Loop {times}") };
                format!("{head} {{\n{inner}{pad}}}")
            }
            Step::WaitPixel { x, y, color, .. } => {
                let bgr = color.trim_start_matches('#');
                format!("while (PixelGetColor({x}, {y}) != \"0x{bgr}\")\n{pad}    Sleep 20")
            }
            Step::FocusRoblox => "WinActivate \"ahk_exe RobloxPlayerBeta.exe\"".into(),
            Step::Run { target } => format!("Run \"{}\"", target.replace('"', "`\"")),
            Step::Comment { text } => format!("; {text}"),
        };
        out.push_str(&pad);
        out.push_str(&line);
        out.push('\n');
    }
}

/// Writes a macro as an AutoHotkey v2 script.
pub fn export(m: &Macro) -> String {
    let mut out = String::new();
    out.push_str(&format!("; {} — exported from Pious\n#Requires AutoHotkey v2.0\nSendMode \"Input\"\nCoordMode \"Mouse\", \"Screen\"\n\n", m.name));
    let mut body = String::new();
    write_steps(&m.steps, 1, &mut body);
    let label = ahk_hotkey(&m.hotkey).unwrap_or_else(|| "F8".into());
    let repeat = match m.repeat {
        Repeat::Once => body,
        Repeat::Times => format!("    Loop {} {{\n{}    }}\n", m.times.max(1), indent(&body)),
        Repeat::UntilStopped | Repeat::WhileHeld => format!("    Loop {{\n{}    }}\n", indent(&body)),
    };
    out.push_str(&format!("{label}:: {{\n{repeat}}}\n"));
    out
}

fn indent(text: &str) -> String {
    text.lines().map(|l| format!("    {l}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_v1_script() {
        let script = "; farm\nF1::\nLoop, 3 {\n  Send, {e down}\n  Sleep, 120\n  Send {e up}\n  Click, 100, 200\n}\nSend ^c\nreturn";
        let out = import(script);
        assert_eq!(out.hotkey.as_deref(), Some("F1"));
        assert_eq!(out.skipped, 0);
        let Step::Loop { times, steps } = &out.steps[0] else { panic!("no loop: {:?}", out.steps) };
        assert_eq!(*times, 3);
        assert_eq!(steps[0], Step::Key { key: "KeyE".into(), press: Press::Down, hold_ms: 0 });
        assert_eq!(steps[1], Step::Wait { ms: 120, random_ms: 0 });
        assert_eq!(steps[3], Step::Move { x: 100, y: 200, relative: false, duration_ms: 0 });
        assert_eq!(steps[4], Step::Click { button: Button::Left, press: Press::Tap, count: 1 });
        // Ctrl+C: down, tap, up.
        assert_eq!(out.steps[1], Step::Key { key: "ControlLeft".into(), press: Press::Down, hold_ms: 0 });
        assert_eq!(out.steps[2], Step::Key { key: "KeyC".into(), press: Press::Tap, hold_ms: 30 });
        assert_eq!(out.steps[3], Step::Key { key: "ControlLeft".into(), press: Press::Up, hold_ms: 0 });
    }

    #[test]
    fn reads_v2_and_keeps_unknown_lines() {
        let out = import("^!a:: {\n    Send \"hi{Enter}\"\n    Sleep(50)\n    MsgBox \"done\"\n}");
        assert_eq!(out.hotkey.as_deref(), Some("Control+Alt+KeyA"));
        assert_eq!(out.steps[0], Step::Text { text: "hi".into(), delay_ms: 0 });
        assert_eq!(out.steps[1], Step::Key { key: "Enter".into(), press: Press::Tap, hold_ms: 30 });
        assert_eq!(out.steps[2], Step::Wait { ms: 50, random_ms: 0 });
        assert!(matches!(&out.steps[3], Step::Comment { .. }));
        assert_eq!(out.skipped, 1);
    }

    #[test]
    fn loops_without_braces_cover_one_line() {
        let out = import("Loop 5
    Click
Send a");
        assert_eq!(out.steps.len(), 2);
        assert!(matches!(&out.steps[0], Step::Loop { times: 5, steps } if steps.len() == 1));
        let out = import("Loop, 2
{
Click
Click
}");
        assert!(matches!(&out.steps[0], Step::Loop { times: 2, steps } if steps.len() == 2));
    }

    #[test]
    fn round_trips() {
        let m = Macro {
            hotkey: "Shift+F2".into(),
            steps: vec![
                Step::Key { key: "KeyW".into(), press: Press::Down, hold_ms: 0 },
                Step::Wait { ms: 500, random_ms: 0 },
                Step::Key { key: "KeyW".into(), press: Press::Up, hold_ms: 0 },
                Step::Click { button: Button::Right, press: Press::Tap, count: 2 },
            ],
            ..Macro::default()
        };
        let back = import(&export(&m));
        assert_eq!(back.hotkey.as_deref(), Some("Shift+F2"));
        assert_eq!(back.steps, m.steps);
    }
}
