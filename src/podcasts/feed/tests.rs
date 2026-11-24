use super::*;

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title>Night &amp; Day</title>
    <image><title>Cover, not the show</title><url>https://example.com/c.jpg</url></image>
    <item>
      <title><![CDATA[Episode 2: <Return>]]></title>
      <guid isPermaLink="false">ep-2</guid>
      <pubDate>Tue, 10 Jun 2025 04:00:00 GMT</pubDate>
      <itunes:duration>1:02:03</itunes:duration>
      <itunes:title>Another title</itunes:title>
      <enclosure url="https://example.com/2.mp3?a=1&amp;b=2" type="audio/mpeg" length="1"/>
    </item>
    <item>
      <title>Episode 1</title>
      <enclosure url="https://example.com/1.mp3" type="audio/mpeg"/>
    </item>
    <item>
      <title>Show notes only</title>
    </item>
    <item>
      <title>Local file</title>
      <enclosure url="file:///etc/passwd"/>
    </item>
  </channel>
</rss>"#;

#[test]
fn an_rss_feed_lists_its_playable_episodes() {
    let feed = parse(RSS).expect("a feed");
    assert_eq!(feed.title, "Night & Day");
    assert_eq!(
        feed.episodes,
        [
            Episode {
                guid: "ep-2".into(),
                title: "Episode 2: <Return>".into(),
                url: "https://example.com/2.mp3?a=1&b=2".into(),
                published_s: Some(1_749_528_000),
                duration_s: Some(3723),
            },
            Episode {
                guid: "https://example.com/1.mp3".into(),
                title: "Episode 1".into(),
                url: "https://example.com/1.mp3".into(),
                published_s: None,
                duration_s: None,
            },
        ]
    );
}

#[test]
fn an_atom_feed_reads_its_enclosure_links() {
    let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom">
        <title>Atom Show</title>
        <id>urn:show</id>
        <entry>
          <id>urn:ep:1</id>
          <link rel="alternate" href="https://example.com/page"/>
          <link rel="enclosure" href="https://example.com/1.ogg"/>
          <published>2025-06-10T04:00:00Z</published>
        </entry>
    </feed>"#;
    let feed = parse(atom).expect("a feed");
    assert_eq!(feed.title, "Atom Show");
    assert_eq!(
        feed.episodes,
        [Episode {
            guid: "urn:ep:1".into(),
            title: UNTITLED.into(),
            url: "https://example.com/1.ogg".into(),
            published_s: Some(1_749_528_000),
            duration_s: None,
        }]
    );
}

#[test]
fn other_documents_are_not_feeds() {
    assert_eq!(parse("<html><body><title>Hi</title></body></html>"), Err(NotAFeed));
    assert_eq!(parse("<rss><channel></channel></rss>"), Err(NotAFeed));
    assert_eq!(parse("not xml at all"), Err(NotAFeed));
}

#[test]
fn a_broken_feed_keeps_what_was_read() {
    let cut = "<rss><channel><title>Cut</title><item><title>A</title>\
               <enclosure url=\"http://e.com/a.mp3\"/></item><item><title>B";
    let feed = parse(cut).expect("a feed");
    assert_eq!(feed.title, "Cut");
    assert_eq!(feed.episodes.len(), 1);
}
