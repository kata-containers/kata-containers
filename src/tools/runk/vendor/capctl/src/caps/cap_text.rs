use core::fmt;

use crate::caps::{CapSet, CapState, NUM_CAPS};

pub fn caps_from_text(s: &str) -> Result<CapState, ParseCapsError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ParseCapsError::InvalidFormat);
    }

    let mut res = CapState::empty();

    for part in s.split_whitespace() {
        update_capstate_single(part, &mut res)?;
    }

    Ok(res)
}

fn update_capstate_single(s: &str, state: &mut CapState) -> Result<(), ParseCapsError> {
    let index = match s.find(|c| c == '+' || c == '-' || c == '=') {
        Some(i) => i,
        None => return Err(ParseCapsError::InvalidFormat),
    };

    if index == 0 && !s.starts_with('=') {
        // Example: "+eip" or "-eip"
        return Err(ParseCapsError::InvalidFormat);
    }

    let spec_caps = parse_capset(&s[..index])?;

    let mut should_raise = true;
    let mut last_ch = None;

    for ch in s[index..].chars() {
        match ch {
            '=' | '+' | '-' => match last_ch {
                // No "+/-/=" following each other
                Some('=') | Some('+') | Some('-') => return Err(ParseCapsError::InvalidFormat),
                _ => (),
            },

            'p' | 'i' | 'e' => debug_assert!(last_ch.is_some()),

            _ => return Err(ParseCapsError::InvalidFormat),
        }

        let set = match ch {
            '=' => {
                state.effective -= spec_caps;
                state.inheritable -= spec_caps;
                state.permitted -= spec_caps;

                should_raise = true;
                None
            }

            '+' => {
                should_raise = true;
                None
            }
            '-' => {
                should_raise = false;
                None
            }

            'p' => Some(&mut state.permitted),
            'i' => Some(&mut state.inheritable),
            'e' => Some(&mut state.effective),

            _ => unreachable!(),
        };

        if let Some(set) = set {
            if should_raise {
                *set |= spec_caps;
            } else {
                *set -= spec_caps;
            }
        }

        last_ch = Some(ch);
    }

    Ok(())
}

fn parse_capset(s: &str) -> Result<CapSet, ParseCapsError> {
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok(!CapSet::empty());
    }

    let mut res = CapSet::empty();

    for part in s.split(',') {
        match part.parse() {
            Ok(cap) => res.add(cap),
            Err(_) => {
                return Err(ParseCapsError::UnknownCapability);
            }
        }
    }

    Ok(res)
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ParseCapsError {
    InvalidFormat,
    UnknownCapability,
    BadFileEffective,
}

impl ParseCapsError {
    fn desc(&self) -> &str {
        match *self {
            Self::InvalidFormat => "Invalid format",
            Self::UnknownCapability => "Unknown capability",
            Self::BadFileEffective => "Effective set must be either empty or same as permitted set",
        }
    }
}

impl fmt::Display for ParseCapsError {
    #[allow(deprecated)]
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.desc())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ParseCapsError {}

pub fn caps_to_text(mut state: CapState, f: &mut fmt::Formatter) -> fmt::Result {
    if state == CapState::empty() {
        return f.write_char('=');
    }

    use core::fmt::Write;

    fn format_capset(f: &mut fmt::Formatter, caps: &CapSet, prefix_ch: char) -> fmt::Result {
        debug_assert!(!caps.is_empty());

        if *caps == !CapSet::empty() {
            // Full set
            if prefix_ch != '=' {
                f.write_str("all")?;
            }
        } else {
            for (i, cap) in caps.iter().enumerate() {
                if i != 0 {
                    f.write_char(',')?;
                }

                f.write_str("cap_")?;

                for ch in cap.name().chars() {
                    f.write_char(ch.to_ascii_lowercase())?;
                }
            }
        }

        Ok(())
    }

    let mut first = true;

    fn format_part(
        f: &mut fmt::Formatter,
        state: &mut CapState,
        drop_state: &mut CapState,
        first: &mut bool,
        effective: bool,
        inheritable: bool,
        permitted: bool,
    ) -> fmt::Result {
        let mut caps = !CapSet::empty();

        debug_assert!(effective || inheritable || permitted);

        if effective {
            caps &= state.effective;
        }
        if inheritable {
            caps &= state.inheritable;
        }
        if permitted {
            caps &= state.permitted;
        }

        if caps.is_empty() {
            return Ok(());
        }

        if NUM_CAPS as usize - caps.size() <= 10 {
            caps = !CapSet::empty();
        }

        let prefix_ch = if *first { '=' } else { '+' };

        if *first {
            *first = false;
        } else {
            f.write_char(' ')?;
        }

        format_capset(f, &caps, prefix_ch)?;

        f.write_char(prefix_ch)?;

        if effective {
            f.write_char('e')?;
            drop_state.effective |= caps - state.effective;
            state.effective -= caps;
        }
        if inheritable {
            f.write_char('i')?;
            drop_state.inheritable |= caps - state.inheritable;
            state.inheritable -= caps;
        }
        if permitted {
            f.write_char('p')?;
            drop_state.permitted |= caps - state.permitted;
            state.permitted -= caps;
        }

        Ok(())
    }

    fn format_part_drop(
        f: &mut fmt::Formatter,
        drop_state: &mut CapState,
        effective: bool,
        inheritable: bool,
        permitted: bool,
    ) -> fmt::Result {
        let mut drop_caps = !CapSet::empty();

        debug_assert!(effective || inheritable || permitted);

        if effective {
            drop_caps &= drop_state.effective;
        }
        if inheritable {
            drop_caps &= drop_state.inheritable;
        }
        if permitted {
            drop_caps &= drop_state.permitted;
        }

        if drop_caps.is_empty() {
            return Ok(());
        }

        f.write_char(' ')?;
        format_capset(f, &drop_caps, '-')?;
        f.write_char('-')?;

        if effective {
            f.write_char('e')?;
            drop_state.effective -= drop_caps;
        }
        if inheritable {
            f.write_char('i')?;
            drop_state.inheritable -= drop_caps;
        }
        if permitted {
            f.write_char('p')?;
            drop_state.permitted -= drop_caps;
        }

        Ok(())
    }

    // This stores the capabilities that need to be *dropped* from each set.
    // For example, if the effective and permitted sets are full except for CAP_CHOWN, we generate
    // `all=ep cap_chown-ep`. Immediately after we generate `all=ep`, drop_state.effective and
    // drop_state.permitted will both be holding just CAP_CHOWN. Later, we revisit that, see that
    // CAP_CHOWN is set, and generate `cap_chown-ep`.
    let mut drop_state = CapState::empty();

    format_part(f, &mut state, &mut drop_state, &mut first, true, true, true)?;

    format_part_drop(f, &mut drop_state, true, true, true)?;

    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        true,
        true,
        false,
    )?;
    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        false,
        true,
        true,
    )?;
    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        true,
        false,
        true,
    )?;

    format_part_drop(f, &mut drop_state, true, true, false)?;
    format_part_drop(f, &mut drop_state, false, true, true)?;
    format_part_drop(f, &mut drop_state, true, false, true)?;

    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        true,
        false,
        false,
    )?;
    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        false,
        true,
        false,
    )?;
    format_part(
        f,
        &mut state,
        &mut drop_state,
        &mut first,
        false,
        false,
        true,
    )?;

    format_part_drop(f, &mut drop_state, true, false, false)?;
    format_part_drop(f, &mut drop_state, false, true, false)?;
    format_part_drop(f, &mut drop_state, false, false, true)?;

    debug_assert_eq!(state, CapState::empty());
    debug_assert_eq!(drop_state, CapState::empty());

    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    use crate::caps::Cap;
    use crate::capset;

    #[test]
    fn test_parse_capset() {
        assert_eq!(parse_capset("").unwrap(), !CapSet::empty());
        assert_eq!(parse_capset("all").unwrap(), !CapSet::empty());
        assert_eq!(parse_capset("ALL").unwrap(), !CapSet::empty());

        assert_eq!(parse_capset("cap_chown").unwrap(), capset!(Cap::CHOWN));
        assert_eq!(parse_capset("CAP_CHOWN").unwrap(), capset!(Cap::CHOWN));
        assert_eq!(
            parse_capset("cap_chown,cap_syslog").unwrap(),
            capset!(Cap::CHOWN, Cap::SYSLOG),
        );

        assert_eq!(
            parse_capset("cap_noexist").unwrap_err().to_string(),
            "Unknown capability"
        );
        assert_eq!(
            parse_capset(",").unwrap_err().to_string(),
            "Unknown capability"
        );
    }

    #[test]
    fn test_parse_capstate() {
        assert_eq!(
            caps_from_text("").unwrap_err().to_string(),
            "Invalid format"
        );
        assert_eq!(
            caps_from_text(" ").unwrap_err().to_string(),
            "Invalid format"
        );

        assert_eq!(
            caps_from_text("cap_chown").unwrap_err().to_string(),
            "Invalid format"
        );

        assert_eq!(
            caps_from_text("+eip").unwrap_err().to_string(),
            "Invalid format"
        );
        assert_eq!(
            caps_from_text("-eip").unwrap_err().to_string(),
            "Invalid format"
        );

        assert_eq!(
            caps_from_text("cap_chown+-p").unwrap_err().to_string(),
            "Invalid format"
        );
        assert_eq!(
            caps_from_text("cap_chown=-p").unwrap_err().to_string(),
            "Invalid format"
        );

        assert_eq!(
            caps_from_text("cap_chown+y").unwrap_err().to_string(),
            "Invalid format"
        );

        assert_eq!(
            caps_from_text("cap_noexist+p").unwrap_err().to_string(),
            "Unknown capability"
        );

        assert_eq!(
            caps_from_text("cap_chown=p").unwrap(),
            CapState {
                permitted: capset!(Cap::CHOWN),
                effective: capset!(),
                inheritable: capset!(),
            }
        );

        assert_eq!(
            caps_from_text("cap_chown+p").unwrap(),
            CapState {
                permitted: capset!(Cap::CHOWN),
                effective: capset!(),
                inheritable: capset!(),
            }
        );

        assert_eq!(
            caps_from_text("cap_chown+ie").unwrap(),
            CapState {
                permitted: capset!(),
                effective: capset!(Cap::CHOWN),
                inheritable: capset!(Cap::CHOWN),
            }
        );

        assert_eq!(
            caps_from_text("=e cap_chown-e").unwrap(),
            CapState {
                permitted: capset!(),
                effective: !capset!(Cap::CHOWN),
                inheritable: capset!(),
            }
        );

        assert_eq!(
            caps_from_text("=e").unwrap(),
            CapState {
                permitted: capset!(),
                effective: !capset!(),
                inheritable: capset!(),
            }
        );

        assert_eq!(
            caps_from_text("all=e").unwrap(),
            CapState {
                permitted: capset!(),
                effective: !capset!(),
                inheritable: capset!(),
            }
        );
    }
}
