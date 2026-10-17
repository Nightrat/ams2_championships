//! How values are written for people to read — dates, lap times, money, track names — wherever
//! they are rendered in Rust (the HTML export), so a lap time or a date reads the same as on the
//! page. The browser has its own copies in `utils.js` (`fmtLapTime`, `fmtDate`, `fmtCredits`,
//! `fmtTrack`); these match them.
//!
//! Presentation only: nothing here decides anything.

pub const DASH: &str = "\u{2014}";

/// `m:ss.mmm`, or `ss.mmm` under a minute — seconds always two digits, as `fmtLapTime` writes
/// them. A dash for no time.
pub fn lap_time(t: f32) -> String {
    if t <= 0.0 {
        return DASH.into();
    }
    let m = (t / 60.0).floor() as u32;
    let s = t - (m as f32) * 60.0;
    if m > 0 {
        format!("{m}:{s:06.3}")
    } else {
        format!("{s:06.3}")
    }
}

/// `YYYY-MM-DD HH:MM` in local time, as `fmtDate` shows a recorded session. Empty for 0, which is
/// "no date".
pub fn date_time(unix_secs: u64) -> String {
    if unix_secs == 0 {
        return String::new();
    }
    let t = local(unix_secs as i64);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        t.year, t.month, t.day, t.hour, t.minute
    )
}

/// `YYYY-MM-DD` in local time — today's, for naming an exported file.
pub fn today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let t = local(now as i64);
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// Whole credits with thousands separators — `fmtCredits`. Negative stays negative: a career can
/// be in debt.
pub fn credits(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// "Track – Layout", or just the track when the layout is empty — `fmtTrack`.
pub fn track(track: &str, variation: &str) -> String {
    if variation.is_empty() {
        track.to_string()
    } else {
        format!("{track} \u{2013} {variation}")
    }
}

/// `1st`, `2nd`, `3rd`, `4th`, `11th`…
pub fn ordinal(n: u32) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A calendar moment, as far as these formats need one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The UTC calendar date and time of a unix timestamp (Howard Hinnant's `civil_from_days`).
pub(crate) fn utc(unix_secs: i64) -> Civil {
    let days = unix_secs.div_euclid(86_400);
    let secs = unix_secs.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    Civil {
        year,
        month,
        day,
        hour: secs / 3600,
        minute: secs % 3600 / 60,
        second: secs % 60,
    }
}

/// [`utc`] moved into the machine's time zone, with the daylight-saving rule in force on that
/// date rather than today — a race recorded in July still reads as summer time in December.
#[cfg(windows)]
fn local(unix_secs: i64) -> Civil {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::Time::SystemTimeToTzSpecificLocalTime;

    let u = utc(unix_secs);
    let universal = SYSTEMTIME {
        wYear: u.year.clamp(1601, 30827) as u16,
        wMonth: u.month as u16,
        wDayOfWeek: 0, // Ignored on input.
        wDay: u.day as u16,
        wHour: u.hour as u16,
        wMinute: u.minute as u16,
        wSecond: u.second as u16,
        wMilliseconds: 0,
    };
    let mut l = universal;
    // SAFETY: both pointers are to live, correctly sized SYSTEMTIMEs; a null zone means "the
    // zone this machine is set to".
    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &universal, &mut l) };
    if ok == 0 {
        return u;
    }
    Civil {
        year: i64::from(l.wYear),
        month: u32::from(l.wMonth),
        day: u32::from(l.wDay),
        hour: u32::from(l.wHour),
        minute: u32::from(l.wMinute),
        second: u32::from(l.wSecond),
    }
}

/// Elsewhere there is no zone lookup without a dependency, and nothing here is recorded there.
#[cfg(not(windows))]
fn local(unix_secs: i64) -> Civil {
    utc(unix_secs)
}

#[cfg(test)]
#[path = "tests/format.rs"]
mod tests;
