use std::fs;

use super::*;

fn drive(name: &str, device: &str, mount: Option<&str>) -> Disc {
    Disc {
        name: name.into(),
        kind: None,
        device: Some(PathBuf::from(device)),
        mount: mount.map(PathBuf::from),
    }
}

fn request(disc: &Disc, kind: DiscKind) -> Request<'_> {
    Request { disc, kind, menus: true, title: None, chapter: None }
}

fn only_device(media: &[Media]) -> &Device {
    match media {
        [Media::Device(device)] => device,
        other => panic!("expected one device, got {other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn dvds_play_from_the_drive_with_or_without_menus_and_from_a_title() {
    let disc = drive("MY_FILM", "/dev/rdisk4", Some("/Volumes/MY_FILM"));
    let with_menus = media(&request(&disc, DiscKind::Dvd), &[]);
    assert_eq!(only_device(&with_menus).mrl(), "dvd:///dev/rdisk4");
    assert_eq!(only_device(&with_menus).display_name(), "MY_FILM");
    let plain =
        Request { menus: false, title: Some(2), chapter: Some(5), ..request(&disc, DiscKind::Dvd) };
    assert_eq!(only_device(&media(&plain, &[])).mrl(), "dvdsimple:///dev/rdisk4#2:5");
}

#[cfg(unix)]
#[test]
fn blu_rays_play_from_their_files_and_say_whether_menus_show() {
    let disc = drive("My Film", "/dev/rdisk4", Some("/Volumes/My Film"));
    let menus = media(&request(&disc, DiscKind::BluRay), &[]);
    assert_eq!(only_device(&menus).mrl(), "bluray:///Volumes/My%20Film");
    assert!(only_device(&menus).options().contains(&":bluray-menu".to_owned()));
    let plain = Request { menus: false, ..request(&disc, DiscKind::BluRay) };
    assert!(only_device(&media(&plain, &[])).options().contains(&":no-bluray-menu".to_owned()));
}

#[cfg(unix)]
#[test]
fn audio_cds_play_one_item_per_track_named_by_cd_text() {
    let disc = drive("Audio CD", "/dev/rdisk4", None);
    let tracks = [
        CdTrack {
            title: Some("So What".into()),
            artist: Some("Miles Davis".into()),
            duration: None,
        },
        CdTrack::default(),
    ];
    let items = media(&request(&disc, DiscKind::AudioCd), &tracks);
    let devices: Vec<_> = items.iter().filter_map(Media::as_device).collect();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].mrl(), "cdda:///dev/rdisk4");
    assert!(devices[0].options().contains(&":cdda-track=1".to_owned()));
    assert!(devices[0].options().contains(&":meta-artist=Miles Davis".to_owned()));
    assert_eq!(devices[0].display_name(), "So What");
    assert_eq!(devices[1].display_name(), "Track 2");
    assert_eq!(media(&request(&disc, DiscKind::AudioCd), &[]), []);
    assert_eq!(audio_cd_mrl(&disc).as_deref(), Some("cdda:///dev/rdisk4"));
}

#[test]
fn a_disc_with_nowhere_to_read_plays_nothing() {
    let disc = Disc { name: "Gone".into(), kind: None, device: None, mount: None };
    assert_eq!(media(&request(&disc, DiscKind::Dvd), &[]), []);
}

#[test]
fn disc_folders_are_told_apart_by_their_folders() {
    let root = std::env::temp_dir().join(format!("rvlc-discs-{}", std::process::id()));
    for (name, inner) in [("dvd", "VIDEO_TS"), ("bluray", "BDMV"), ("vcd", "MPEGAV"), ("none", "x")]
    {
        fs::create_dir_all(root.join(name).join(inner)).expect("a temporary folder");
    }
    assert_eq!(kind_in(&root.join("dvd")), Some(DiscKind::Dvd));
    assert_eq!(kind_in(&root.join("dvd/VIDEO_TS")), Some(DiscKind::Dvd));
    assert_eq!(kind_in(&root.join("bluray")), Some(DiscKind::BluRay));
    assert_eq!(kind_in(&root.join("vcd")), Some(DiscKind::Vcd));
    assert_eq!(kind_in(&root.join("none")), None);
    assert_eq!(Disc::picked(&root.join("dvd")).kind, Some(DiscKind::Dvd));
    fs::remove_dir_all(&root).ok();
}
