# Subtitles in fourteen encodings (S9.8.1, S6.1.6): the four Unicode forms libVLC reads by
# itself, and ten legacy code pages the app has to guess. Each is a short SRT of four lines of
# dialogue, long enough for the guess; the manifest names the encoding the app should land on
# (encoding_rs's name) and the first line, which it should show.

# subtitle NAME ICONV_CHARSET EXPECTED_ENCODING BOM: writes subtitles/NAME.srt from the four
# lines on stdin, converted with iconv, after BOM (printf escapes, or empty).
subtitle() {
  local name="$1" charset="$2" encoding="$3" bom="$4"
  local path="subtitles/$name.srt" utf8 first number=0 line
  utf8="$(mktemp -t subtitle)"
  while IFS= read -r line; do
    if [[ $number -eq 0 ]]; then first="$line"; fi
    number=$((number + 1))
    printf '%d\r\n00:00:0%d,000 --> 00:00:0%d,900\r\n%s\r\n\r\n' \
      "$number" "$((number * 2))" "$((number * 2 + 1))" "$line" >>"$utf8"
  done
  mkdir -p "$out/subtitles"
  { printf '%b' "$bom"; iconv -f UTF-8 -t "$charset" "$utf8"; } >"$out/$path"
  rm -f "$utf8"
  entry "$path" subtitle reads "encoding=$(toml_string "$encoding")" "text=$(toml_string "$first")"
}

subtitle utf-8 UTF-8 UTF-8 '' <<'EOF'
Ça va très bien — 你好 — Привет — שלום
Mixed scripts are only possible in Unicode.
The app should read this file as it is.
Nothing here needs converting.
EOF

subtitle utf-8-bom UTF-8 UTF-8 '\xef\xbb\xbf' <<'EOF'
Tiếng Việt có dấu thanh rất đặc biệt.
Tối qua bạn đã ở đâu vậy?
Tôi đã đợi bạn đến tận sáng.
Thôi được, chúng ta xem phim đi.
EOF

subtitle utf-16le-bom UTF-16LE UTF-16LE '\xff\xfe' <<'EOF'
नमस्ते, आप कैसे हैं?
तुम पूरी रात कहाँ थे?
मैं सुबह तक तुम्हारा इंतज़ार करती रही।
ठीक है, चलो फ़िल्म देखते हैं।
EOF

subtitle utf-16be-bom UTF-16BE UTF-16BE '\xfe\xff' <<'EOF'
สวัสดีครับ ยินดีที่ได้รู้จัก
เมื่อคืนคุณไปอยู่ที่ไหนมา
ฉันรอคุณจนถึงเช้าเลยนะ
ก็ได้ งั้นเราดูหนังกันเถอะ
EOF

subtitle windows-1252-french WINDOWS-1252 windows-1252 '' <<'EOF'
Où est passée la clé de la voiture ?
Je crois qu'elle est restée dans la poche de ton manteau.
Très bien, alors dépêchons-nous, le film commence à huit heures.
Ça ne fait rien, on prendra le métro.
EOF

subtitle windows-1250-czech CP1250 windows-1250 '' <<'EOF'
Příliš žluťoučký kůň úpěl ďábelské ódy.
Kde jsi byl celý večer? Čekala jsem na tebe.
Řekl jsem ti, že přijdu později, protože jsem musel pracovat.
Dobře, ale příště mi prosím zavolej.
EOF

subtitle windows-1251-russian CP1251 windows-1251 '' <<'EOF'
Где ты был всю ночь?
Я ждала тебя до самого утра и очень волновалась.
Прости, телефон разрядился, а такси так и не приехало.
Ладно, давай просто посмотрим фильм.
EOF

subtitle windows-1253-greek CP1253 windows-1253 '' <<'EOF'
Πού ήσουν όλο το βράδυ;
Σε περίμενα μέχρι το πρωί και ανησυχούσα πολύ.
Συγγνώμη, το κινητό μου έκλεισε και το ταξί δεν ήρθε ποτέ.
Εντάξει, ας δούμε απλώς την ταινία.
EOF

subtitle windows-1255-hebrew CP1255 windows-1255 '' <<'EOF'
איפה היית כל הלילה?
חיכיתי לך עד הבוקר ודאגתי מאוד.
סליחה, הטלפון שלי נכבה והמונית לא הגיעה.
בסדר, בוא פשוט נראה את הסרט.
EOF

subtitle windows-1256-arabic CP1256 windows-1256 '' <<'EOF'
أين كنت طوال الليل؟
انتظرتك حتى الصباح وكنت قلقة جداً.
آسف، نفدت بطارية هاتفي ولم تصل سيارة الأجرة.
حسناً، لنشاهد الفيلم فقط.
EOF

subtitle shift_jis-japanese SHIFT_JIS Shift_JIS '' <<'EOF'
昨日の夜はどこにいたの？
朝までずっと待っていて、とても心配したよ。
ごめん、携帯の電池が切れて、タクシーも来なかったんだ。
まあいいや、映画を見よう。
EOF

subtitle gbk-chinese-simplified GBK GBK '' <<'EOF'
你昨天晚上去哪儿了？
我一直等到早上，非常担心你。
对不起，我的手机没电了，出租车也一直没来。
好吧，我们还是看电影吧。
EOF

subtitle big5-chinese-traditional BIG5 Big5 '' <<'EOF'
你昨天晚上去哪裡了？
我一直等到早上，非常擔心你。
對不起，我的手機沒電了，計程車也一直沒來。
好吧，我們還是看電影吧。
EOF

subtitle euc-kr-korean EUC-KR EUC-KR '' <<'EOF'
어젯밤에 어디 있었어?
아침까지 계속 기다렸고 정말 걱정했어.
미안해, 휴대폰 배터리가 다 됐고 택시도 안 왔어.
알았어, 그냥 영화나 보자.
EOF
