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
    // The names in order, each with its value, `format` left out, then the secret.
    let signed = concat!(
        "api_key",
        "KEY",
        "method",
        "auth.getMobileSession",
        "password",
        "pw",
        "username",
        "ana",
        "SECRET",
    );
    assert_eq!(signature(&parameters, "SECRET"), hex(&Md5::digest(signed)));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        // Writing to a `String` can't fail.
        let _ = write!(hex, "{byte:02x}");
        hex
    })
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

/// The forms a [`Recorder`] was given.
type Forms = std::sync::Arc<std::sync::Mutex<Vec<Vec<(String, String)>>>>;

/// A transport that keeps each form it is given and answers `answer`.
struct Recorder {
    answer: Result<(u16, String), Failure>,
    forms: Forms,
}

impl Transport for Recorder {
    fn post(&self, url: &str, form: &[(String, String)]) -> Result<(u16, String), Failure> {
        assert_eq!(url, URL);
        self.forms.lock().unwrap().push(form.to_vec());
        self.answer.clone()
    }
}

const URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// A client answering `answer`, and the forms it sends.
fn answering(answer: Result<(u16, &str), Failure>) -> (Client, Recorder) {
    let forms = std::sync::Arc::default();
    let answer = answer.map(|(status, body)| (status, body.to_owned()));
    let recorder = Recorder { answer: answer.clone(), forms: std::sync::Arc::clone(&forms) };
    let keys = Keys { key: "KEY".into(), secret: "SECRET".into() };
    let client = Client::with(Box::new(Recorder { answer, forms }), URL, keys);
    (client, recorder)
}

fn value<'a>(form: &'a [(String, String)], name: &str) -> Option<&'a str> {
    form.iter().find(|(field, _)| field == name).map(|(_, value)| value.as_str())
}

fn play(n: i64) -> Play {
    Play {
        artist: "Björk".into(),
        title: format!("Track {n}"),
        album: None,
        duration_s: None,
        started_s: n,
    }
}

#[test]
fn signing_in_sends_a_signed_call_and_answers_the_session_key() {
    let (client, sent) = answering(Ok((200, r#"{"session":{"name":"ana","key":"abc"}}"#)));
    assert_eq!(client.sign_in("ana", "pw"), Ok("abc".into()));
    let forms = sent.forms.lock().unwrap();
    let form = &forms[0];
    assert_eq!(value(form, "method"), Some("auth.getMobileSession"));
    assert_eq!(value(form, "username"), Some("ana"));
    assert_eq!(value(form, "api_key"), Some("KEY"));
    assert_eq!(value(form, "format"), Some("json"));
    let unsigned: Vec<_> = form.iter().filter(|(name, _)| name != "api_sig").cloned().collect();
    assert_eq!(value(form, "api_sig"), Some(signature(&unsigned, "SECRET").as_str()));
}

#[test]
fn a_refused_sign_in_is_a_rejected_login() {
    let (client, _) = answering(Ok((400, r#"{"error":6,"message":"Invalid parameters"}"#)));
    assert_eq!(client.sign_in("ana", "pw"), Err(Failure::LoginRejected));
    let (client, _) = answering(Ok((200, "{}")));
    assert_eq!(client.sign_in("ana", "pw"), Err(Failure::Unreadable), "no session in it");
}

#[test]
fn scrobbling_sends_a_batch_at_most_with_the_session() {
    let (client, sent) = answering(Ok((200, r#"{"scrobbles":{}}"#)));
    let plays: Vec<Play> = (0..MAX_BATCH as i64 + 5).map(play).collect();
    assert_eq!(client.scrobble("session", &plays), Ok(()));
    let forms = sent.forms.lock().unwrap();
    let form = &forms[0];
    assert_eq!(value(form, "method"), Some("track.scrobble"));
    assert_eq!(value(form, "sk"), Some("session"));
    let last = MAX_BATCH - 1;
    assert!(value(form, &format!("track[{last}]")).is_some());
    assert_eq!(value(form, &format!("track[{MAX_BATCH}]")), None, "the rest wait");
}

#[test]
fn a_transport_failure_reaches_the_caller() {
    let (client, _) = answering(Err(Failure::Offline("no route".into())));
    assert_eq!(client.scrobble("session", &[play(1)]), Err(Failure::Offline("no route".into())));
}
