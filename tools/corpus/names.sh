# Odd file names (S9.8.1): one tiny clip under names that break naive path, URL and shell
# handling. names/ holds names every OS allows; names-unix/ holds names Windows refuses, which
# fetch.sh leaves out there.

make_once reference/tiny.mp4 -f lavfi -i "testsrc2=size=320x240:rate=25:duration=2" \
  -c:v libx264 -preset veryfast -pix_fmt yuv420p -movflags +faststart

# named FOLDER NAME [KEY=TOML_VALUE...]: the tiny clip as FOLDER/NAME.
named() {
  local folder="$1" name="$2"
  shift 2
  mkdir -p "$out/$folder"
  cp "$out/reference/tiny.mp4" "$out/$folder/$name"
  entry "$folder/$name" media plays 'seconds=2' "$@"
}

named names 'with spaces.mp4'
named names '  leading spaces.mp4'
named names '-leading-dash.mp4'
named names '#hash & ampersand %20 percent.mp4'
named names "semi;colon,comma'apostrophe [brackets] {braces}.mp4"
named names 'many.dots.in.the.name.v2.final.mp4'
named names 'UPPER CASE.MP4'
named names 'no-extension'
named names 'Ünïcödé café.mp4'
named names "$(printf 'cafe\xcc\x81 decomposed (NFD).mp4')"
named names 'שלום עולם.mp4'
named names 'مرحبا بالعالم.mp4'
named names '日本語のファイル名.mp4'
named names '한국어 파일.mp4'
named names 'emoji 🎬🍿 film.mp4'
named names "$(printf 'zero\xe2\x80\x8bwidth and right\xe2\x80\x8fto\xe2\x80\x8eleft marks.mp4')"
named names "$(printf '%0200d' 0 | tr 0 a).mp4"

# A path past Windows' old 260-character limit.
deep="names/deep"
for level in 1 2 3 4 5 6 7 8 9 10; do
  deep="$deep/folder-number-$level-with-a-long-name"
done
named "$deep" 'clip.mp4'

# A film with its subtitles beside it, the name full of punctuation the matching must survive.
film="Movie – Director's Cut (2024) [1080p]"
cp "$out/subtitles/windows-1252-french.srt" "$out/names/$film.fr.srt"
cp "$out/subtitles/utf-8.srt" "$out/names/$film.en.srt"
named names "$film.mp4" "subtitles=[$(toml_string "$film.en.srt"), $(toml_string "$film.fr.srt")]"

# Names Windows refuses: reserved characters, a reserved device name, a trailing dot, a
# backslash and a tab.
named names-unix 'question? colon: star*.mp4'
named names-unix '"double quotes" <angle> |pipe|.mp4'
named names-unix 'back\slash.mp4'
named names-unix 'CON.mp4'
named names-unix 'trailing dot.mp4.'
named names-unix "$(printf 'tab\there.mp4')"
