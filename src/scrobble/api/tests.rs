use super::*;

fn pairs(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    owned(pairs)
}

#[test]
fn a_signature_hashes_the_sorted_parameters_and_the_secret() {
    let parameters = pairs(&[
        ("username", "ana"),
        ("method", "auth.getMobileSession"),
        ("password", "pw"),
        ("api_key", "KEY"),
        ("format", "json"),
    ]);
    assert_eq!(signature(&parameters, "SECRET"), "f9ce02fee977698eb100ccde38dbaa52");
}

#[test]
fn a_play_sends_its_indexed_fields() {
    let play = Play {
        artist: "Björk".into(),
        title: "Jóga".into(),
        album: Some("Homogenic".into()),
        duration_s: Some(305),
        started_s: 1_700_000_000,
    };
    assert_eq!(
        play_parameters(3, &play),
        pairs(&[
            ("artist[3]", "Björk"),
            ("track[3]", "Jóga"),
            ("timestamp[3]", "1700000000"),
            ("album[3]", "Homogenic"),
            ("duration[3]", "305"),
        ])
    );
    let bare = Play { album: None, duration_s: None, ..play };
    assert_eq!(play_parameters(0, &bare).len(), 3);
}

#[test]
fn answers_read_as_sessions_or_failures() {
    let session = read(200, r#"{"session":{"name":"ana","key":"abc","subscriber":0}}"#);
    assert_eq!(session.ok().and_then(|answer| answer.session).map(|s| s.key), Some("abc".into()));
    let failure = |status, text| read(status, text).err();
    assert_eq!(failure(403, r#"{"error":4,"message":"Wrong"}"#), Some(Failure::LoginRejected));
    assert_eq!(failure(403, r#"{"error":9,"message":"Bad"}"#), Some(Failure::SessionExpired));
    assert_eq!(failure(503, r#"{"error":16,"message":"Later"}"#), Some(Failure::Unavailable));
    assert_eq!(failure(403, r#"{"error":26,"message":"Gone"}"#), Some(Failure::KeyRejected));
    assert_eq!(
        failure(400, r#"{"error":6,"message":"Bad artist"}"#),
        Some(Failure::Refused("Bad artist".into()))
    );
    assert_eq!(failure(502, "<html>Bad gateway</html>"), Some(Failure::Unavailable));
    assert_eq!(failure(200, "nonsense"), Some(Failure::Unreadable));
    assert!(read(200, r#"{"scrobbles":{"@attr":{"accepted":1,"ignored":0}}}"#).is_ok());
}
