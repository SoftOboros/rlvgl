use rlvgl_network::{WifiAccessPoint, WifiScan, WifiSecurity, WifiSsid};

fn observation(id: u8, name: &str, security: WifiSecurity, rssi: i8) -> WifiAccessPoint {
    WifiAccessPoint {
        ssid: WifiSsid::new(name).unwrap(),
        bssid: [id; 6],
        channel: id,
        rssi,
        security,
    }
}

#[test]
fn refreshed_protected_unknown_or_hidden_radio_cannot_reuse_its_old_open_candidate() {
    for (name, security) in [
        ("guest", WifiSecurity::Protected),
        ("guest", WifiSecurity::Unknown),
        ("", WifiSecurity::Open),
    ] {
        let mut scan = WifiScan::<2>::new();
        scan.insert(observation(1, "guest", WifiSecurity::Open, -20));
        scan.insert(observation(2, "other", WifiSecurity::Open, -40));
        scan.insert(observation(1, name, security, -80));
        assert_eq!(scan.len(), 2);
        let candidates: Vec<_> = scan.open_candidates().map(|ap| ap.bssid).collect();
        assert_eq!(candidates, [[2; 6]]);
        let refreshed = scan.iter().find(|ap| ap.bssid == [1; 6]).unwrap();
        assert_eq!(refreshed.ssid.as_str(), name);
        assert_eq!(refreshed.security, security);
    }
}
