# Helpers the corpus scripts share (S9.8.1). Sourced by make.sh, which sets $out (the corpus
# folder) and $manifest (its manifest.toml) before sourcing the section scripts.

ff=(ffmpeg -hide_banner -loglevel error -y)

# toml_string TEXT: TEXT as a TOML basic string, quotes included.
toml_string() {
  local text="${1//\\/\\\\}"
  text="${text//\"/\\\"}"
  printf '"%s"' "$text"
}

# entry PATH KIND EXPECT [KEY=TOML_VALUE...]: adds a file to the manifest. PATH is relative to
# the corpus folder; KIND is media, subtitle or playlist; EXPECT is what the app should do with
# it: plays, fails, reads (a text file decoded), or skipped (a file the app leaves alone).
entry() {
  local path="$1" kind="$2" expect="$3"
  shift 3
  {
    echo
    echo '[[file]]'
    echo "path = $(toml_string "$path")"
    echo "kind = \"$kind\""
    echo "expect = \"$expect\""
    local pair
    for pair in "$@"; do
      echo "${pair%%=*} = ${pair#*=}"
    done
  } >>"$manifest"
}

# make_once NAME FFMPEG_ARGS...: runs ffmpeg into $out/NAME unless it already exists, so
# reruns are cheap.
make_once() {
  local name="$1"
  shift
  if [[ -s "$out/$name" ]]; then return; fi
  echo "making $name"
  mkdir -p "$(dirname "$out/$name")"
  "${ff[@]}" "$@" "$out/$name"
}

# random_bytes FILE COUNT: COUNT random bytes into FILE, unless it exists.
random_bytes() {
  if [[ -e "$1" ]]; then return; fi
  head -c "$2" /dev/urandom >"$1"
}
