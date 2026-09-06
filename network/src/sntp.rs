//! Minimal validated SNTP request and response handling.

/// Length of the fixed SNTP/NTP header used by this client.
pub const NTP_PACKET_LEN: usize = 48;

const NTP_UNIX_EPOCH_DELTA: u64 = 2_208_988_800;
const NTP_ERA_SECONDS: u64 = 1_u64 << 32;

/// A validated time sample from an SNTP server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetworkTime {
    /// Milliseconds since 1970-01-01 00:00:00 UTC.
    pub unix_millis: u64,
    /// Server stratum reported by the response.
    pub stratum: u8,
}

/// Reasons an SNTP response cannot be used as a clock sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NtpError {
    /// The datagram does not contain the complete 48-byte NTP header.
    TooShort,
    /// The server's leap indicator says its clock is not synchronized.
    Unsynchronized,
    /// The response is not NTP version 3 or 4.
    UnsupportedVersion(u8),
    /// The packet is not a server response.
    UnexpectedMode(u8),
    /// Stratum zero or an invalid high stratum was returned.
    InvalidStratum(u8),
    /// The server did not provide a transmit timestamp.
    MissingTransmitTime,
}

/// Construct a minimal SNTPv4 client request.
pub const fn ntp_request() -> [u8; NTP_PACKET_LEN] {
    let mut packet = [0_u8; NTP_PACKET_LEN];
    packet[0] = 0x23;
    packet
}

/// Validate an SNTP response and extract its transmit timestamp.
///
/// NTP seconds wrap into era 1 in 2036. Values below the Unix epoch offset are
/// interpreted as era 1, keeping this client useful across that rollover when
/// it has no preexisting real-time clock.
pub fn parse_ntp_response(packet: &[u8]) -> Result<NetworkTime, NtpError> {
    if packet.len() < NTP_PACKET_LEN {
        return Err(NtpError::TooShort);
    }

    let leap = packet[0] >> 6;
    if leap == 3 {
        return Err(NtpError::Unsynchronized);
    }

    let version = (packet[0] >> 3) & 0x07;
    if !(3..=4).contains(&version) {
        return Err(NtpError::UnsupportedVersion(version));
    }

    let mode = packet[0] & 0x07;
    if mode != 4 {
        return Err(NtpError::UnexpectedMode(mode));
    }

    let stratum = packet[1];
    if !(1..=15).contains(&stratum) {
        return Err(NtpError::InvalidStratum(stratum));
    }

    let seconds = u32::from_be_bytes(packet[40..44].try_into().unwrap()) as u64;
    let fraction = u32::from_be_bytes(packet[44..48].try_into().unwrap()) as u64;
    if seconds == 0 && fraction == 0 {
        return Err(NtpError::MissingTransmitTime);
    }

    let full_ntp_seconds = if seconds < NTP_UNIX_EPOCH_DELTA {
        seconds + NTP_ERA_SECONDS
    } else {
        seconds
    };
    let unix_seconds = full_ntp_seconds - NTP_UNIX_EPOCH_DELTA;
    let fraction_millis = (fraction * 1_000) >> 32;

    Ok(NetworkTime {
        unix_millis: unix_seconds * 1_000 + fraction_millis,
        stratum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(unix_seconds: u64, fraction: u32) -> [u8; NTP_PACKET_LEN] {
        let mut packet = [0_u8; NTP_PACKET_LEN];
        packet[0] = 0x24;
        packet[1] = 2;
        let ntp_seconds = (unix_seconds + NTP_UNIX_EPOCH_DELTA) as u32;
        packet[40..44].copy_from_slice(&ntp_seconds.to_be_bytes());
        packet[44..48].copy_from_slice(&fraction.to_be_bytes());
        packet
    }

    #[test]
    fn request_is_an_sntp_v4_client_packet() {
        let packet = ntp_request();
        assert_eq!(packet.len(), NTP_PACKET_LEN);
        assert_eq!(packet[0], 0x23);
        assert!(packet[1..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn response_extracts_seconds_fraction_and_stratum() {
        let packet = response(1_700_000_000, 0x8000_0000);
        assert_eq!(
            parse_ntp_response(&packet),
            Ok(NetworkTime {
                unix_millis: 1_700_000_000_500,
                stratum: 2,
            })
        );
    }

    #[test]
    fn response_rejects_unusable_server_state() {
        let mut packet = response(1_700_000_000, 0);
        packet[0] = 0xe4;
        assert_eq!(parse_ntp_response(&packet), Err(NtpError::Unsynchronized));

        packet = response(1_700_000_000, 0);
        packet[0] = 0x23;
        assert_eq!(
            parse_ntp_response(&packet),
            Err(NtpError::UnexpectedMode(3))
        );

        packet = response(1_700_000_000, 0);
        packet[1] = 0;
        assert_eq!(
            parse_ntp_response(&packet),
            Err(NtpError::InvalidStratum(0))
        );
    }
}
