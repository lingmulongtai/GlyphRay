# GlyphRay Development Diary

## 2026-06-22 JST - Encrypted Windows/Android Live Session And Enforced Device Permissions

今日はsecurity foundationを実際のWindows/Android sessionへ接続した。Windows hostはDPAPIで永続化したP-256 identityを持ち、承認後に署名付きephemeral ECDH offerを送る。Androidはhost署名を検証し、host idごとにidentity fingerprintをpinし、Android Keystore identityでclient ephemeral keyを署名する。両側はhandshake transcriptからhost-to-client / client-to-hostの別々のAES-256-GCM鍵を導出し、control、video、stylus、touch、keyboard、mouse、gamepadの`GLYT` datagram全体を`GLYE`へ封入するようになった。

UDPの並べ替えを許容しながらduplicateと古いcounterを拒否する4096 packetのreplay windowも両実装に入れた。鍵確立後のplaintextはWindows/Android双方で拒否し、Windowsはsecure sessionが完成するまでvideoをqueueせず、Androidはsecure codecが無ければrealtime inputを送らない。RustとKotlinには同じ固定ECDH後secret/transcriptから同一の方向別鍵を得るcross-platform vector testを置いた。

もう一つ、host settingsに保存されていた端末別pen/touch/keyboard/mouse/gamepad permissionをrouterの実効gateへ接続した。`trust permission <id> <kind> <on|off>`で保存値とactive sessionを同時に変更でき、denyされたinputはdecodeやWin32 injectionより前で落ちる。ここまで安全境界が揃ったのでvideoとnative inputは標準起動へ変更し、必要な診断時だけ`GLYPHRAY_DISABLE_*`で止める方式にした。

検証は`cargo fmt --check`相当、strict Clippy、Rust workspace全test、Windows host 44 library tests + 8 CLI tests、Windows release build、Android unit tests、debug APK、release APK/AAB、release lintまで成功した。残るsecurity課題はmacOSの同じ`GLYH`/`GLYE`統合、初回pairingのQR/数値によるout-of-band確認、real Android/interactive Windowsでの長時間replay・reconnect・lock/unlock試験である。

現在の実装進捗は90%、製品release準備は72%。

## 2026-06-22 JST - DXGI Desktop Duplication And Full Video Datagram Probe

今日はWindows映像経路からGDI `BitBlt`を外し、DXGI Desktop Duplicationへ置き換えた。capture sessionはdisplayごとにD3D11 device、output duplication、CPU-readable staging textureを保持する。GPU textureのrow pitchを尊重してBGRAを読み戻し、90/180/270度のdisplay rotationを補正し、画面に変化がないtimeoutでは直前frameを再利用する。display変更やlock/unlockで`DXGI_ERROR_ACCESS_LOST`になった場合はsessionを作り直す。

monitor metadataも固定60Hzから実値へ変えた。この端末ではDXGI/GDI metadataから2560x1440 165Hzのprimary displayと、2560x1440 120Hzのsecondary displayを検出した。現在のCodex automation desktopは`DuplicateOutput`を`0x80070005`で拒否したため実pixel captureは通常のinteractive sessionで再検証が必要だが、列挙、adapter/device作成、capture APIのcompile pathは通っている。

`glyphray-encoder-diagnostics`は合成frame fallbackを使い、1280x720 BGRAをMedia Foundationで5,563-byteのAnnex B H.264 keyframeへ4.114msでencodeした。そのaccess unitを1200-byte payloadで5個のGLYT UDP datagramへ分割し、wire encode/decode後に再構築してCRC32 `5eb11eb4`の完全一致まで確認した。Rust workspaceはstrict Clippyと全testが成功し、Windows host testはDesktop Duplicationの回転testを含む41件になった。

現在の実装進捗は87%、製品release準備は67%。次の大きなrelease blockerはlive session encryption、hardware encoder選択、interactive Windows/実Androidの連続stream検証である。

## 2026-06-22 JST - Real Windows Media Foundation H.264 Encoder

release pipelineを整えた後、最大の映像blockerだった`PendingHardwareEncoder`を実装へ置き換えた。WindowsではMicrosoft標準のMedia Foundation H.264 encoder MFTを使い、GDIのBGRA frameをNV12へ変換し、low-latency mode、LowDelayVBRからCBRへの互換fallback、B-frame無効化、GOP/keyframe制御を設定する。出力がAVCC length-prefixならAnnex Bへ正規化し、既存のVideo packetizerへそのまま渡せる。

`glyphray-encoder-diagnostics`も追加し、このWindows端末で実行した。automated desktop sessionではGDI `BitBlt`がinvalid handleになったためsynthetic 1280x720 BGRAへ切り替えたが、最新runでMedia Foundationは5,563-byteのAnnex B keyframeを2.576msで生成した。これで「空payloadをpacketizeするplaceholder」ではなく、実H.264 access unitをapproved peer queueへ流せる。

現在の実装進捗は86%、製品release準備は66%。次の映像課題はWindows Graphics Capture/Desktop Duplication、hardware MFT選択、Android実機とのcontinuous 1080p60検証。

## 2026-06-22 JST - Release Candidate Pipeline And Honest Release Gates

今日は「リリースできる状態」を感覚ではなく成果物で判定できるようにした。rootの `VERSION` を単一のversion sourceにし、Androidはrelease APK/AAB、WindowsはWiX v4 MSI、macOSは通常の `.app` bundleと `.pkg`、zipを同じversionで作る構成へ変更した。GitHub Actionsには `Release Candidate` workflowを追加し、3 platformのartifactと `SHA256SUMS.txt` を一括生成する。

署名も「あとで入れる」だけではなく、CI secretがあればAndroid signing、Windows Authenticode、macOS Developer ID signingとnotarizationへ進む経路を実装した。手動workflowはunsigned engineering candidateを許すが、`vX.Y.Z` tagからの公開はAndroid/Windows/macOSの署名とmacOS notarizationが全部確認できない限り止まる。誤ってunsigned buildを正式releaseにする道を閉じた。

WindowsではWiX 4.0.6を導入し、`GlyphRayHost-0.1.0.msi` を実生成した。Androidもunit test、release APK、release AAB、release lintが成功した。Rustはworkspace全testに加えて `cargo fmt --check` と `cargo clippy --workspace --all-targets -- -D warnings` をCI gateにし、そこで見つかったWin32固有コード19件のlint問題を修正した。

進捗率も見直した。以前の98%は「基盤が存在する割合」で、実映像encoder、暗号化live session、署名、実機pen検証を軽く見積もっていた。現在は実装進捗84%、製品release準備64%とする。数字は下がったが、完成条件は以前より明確で、release候補を毎回同じ方法で作れるようになった。

## 2026-05-24 JST - macOS Stream Backpressure And Audio Permission

今日は macOS host の連続UDP映像送信を低遅延寄りに固めた。`MacUdpVideoPublisher` に in-flight datagram cap を入れ、送信待ちが一定数を超えたら古い遅延を抱え込まずに video datagram を drop として数えるようにした。snapshot には scheduled / sent / dropped datagram、bytes、in-flight、high watermark、last error が出るので、`Stop Stream` 後に「送れているのか、詰まって落としているのか」がUIから見える。

あわせて、pairing/control runtime が覚えた最新 approved Android endpoint へ直接配信する `Start Approved Stream` も足した。まだ encrypted session ではないが、手入力IPだけのprobeから「承認済みclientへ送る」操作に寄った。

macOS の audio permission request も UI に足した。まだ audio capture / Android playback までは行っていないが、first-run onboarding に必要な Screen Recording、Accessibility、Audio の request path が並び、permission readiness の穴が少し塞がった。macOS host は signed trusted reconnect、capture、encode、packetize、bounded send、permission request まで来たので、次は encrypted session transport と real Android receiver loopback をつなぐ段階。

この時点の進捗見積もり: 98%。

## 2026-05-18 JST - macOS Signed Trusted Reconnect

今日は macOS host の trusted client persistence を、単にKeychainへ保存するだけの状態から一段進めた。Android の Keystore public key DER から SHA-256 fingerprint と `trusted-key-...` id を作り、初回pairing時にKeychainへ保存する。次に同じAndroidがpairingしてきた場合、macOS host は即承認せず `AuthChallenge` を返し、Android の `SHA256withECDSA` 署名付き `AuthResponse` を CryptoKit で検証してから `PairingResult accepted=true` を返す流れにした。

SwiftUI 側にも pending auth challenge の数と、trusted client が public key を持っているかを見えるようにした。これで Windows 版に入っている「戻ってきた端末を公開鍵で確認してから承認する」思想が、macOS の軽量 control runtime にも入った。まだ macOS CI と実機Androidでの検証、encrypted session transport、reconnect、backpressure ownership は残るが、macOS host はただのprobeからかなり実セッションらしくなった。

この時点の進捗見積もり: 98%。

## 2026-05-18 JST - macOS Trusted Client Persistence

今日は macOS host の control runtime に、Keychain-backed な trusted client list を足した。Android から `PairingRequest` が来て `PairingResult` を返したあと、その client id、device name、endpoint、public-key fingerprint を `MacTrustedClientStore` 経由で Keychain に保存する。host 起動時には保存済み client を復元し、最新 endpoint を video target に戻せるようにした。

SwiftUI には `Clear Trust` も追加した。まだこれは「本番の信頼」ではなく、ローカル再接続を楽にするための永続化で、署名付き challenge/response と encrypted transport はこれから。ただ、毎回pairingして初期状態に戻るだけのprobeからは一歩抜け、macOS host が client state を持つようになった。

この時点の進捗見積もり: 98%。

## 2026-05-18 JST - macOS Lightweight Control Pairing Runtime

今日は macOS host を「手入力でUDP送信するだけ」から一歩進め、Android client からの manual host 接続を受けられる軽量 control runtime を追加した。`MacControlRuntime` は `44999/UDP` で `GLYT` Control datagram と `GLYR` protocol frame を読み、`PairingRequest` を受けると `PairingResult` を返す。さらに `LatencyPing` への `LatencyPong` と、Android から送られる `EncoderConfig` の記録にも対応した。

SwiftUI には `Start Control` / `Stop Control` と approved client 表示を追加した。Android が macOS host に pairing request を送ると、macOS 側はその送信元 endpoint を approved client として覚え、UDP video target 欄へ自動反映する。これで `Start UDP Stream` は、手でIP/portを入れなくても、直前にpairingしてきたAndroidへ映像datagramを送る流れに近づいた。

まだ trusted-device identity、暗号化済みsession、LAN discovery advertisement、reconnect/backpressure は未完成。けれど、macOS側の「Androidから接続される、承認する、そこへ映像を返す」という骨格が見えた。

続けて `MacLanDiscoveryAdvertiser` も入れた。`Start Control` を押すと `GLYD` discovery advertisement も送るので、broadcast が通るLANなら Android の host list に macOS host が出る道筋ができた。Windows Ink support は false、H.264 と pairing required は true として広告する。これで手動IP入力だけに頼る段階から少し抜けた。

この時点の進捗見積もり: 97%。

## 2026-05-18 JST - macOS Continuous UDP Video Stream

今日は macOS host の映像経路を、単発の `UDP Send Probe` から連続送信に近づけた。`MacUdpVideoPublisher` を追加し、`Start UDP Stream` / `Stop Stream` で `ScreenCaptureKit -> VideoToolbox -> GLYF/GLYT packetizer -> UDP publisher` を起動・停止できるようにした。まだ approved-client session runtime ではなく手入力の host / port 宛てだが、H.264 frame を継続的に Video channel datagram として出し続ける入口ができた。

ついでに重要な形式差も潰した。VideoToolbox のH.264出力は長さprefix付きNALになりやすいので、sender側でAnnex Bへ変換し、keyframeにはSPS/PPSを付けるようにした。AndroidのMediaCodec受信経路と接続するときに、ここが未処理だと「packetは届くがdecodeできない」状態になりやすいので、先に整えた。

次はこの manual target を Android client の pairing/control runtime から得た approved endpoint に差し替え、reconnect、backpressure、receiver acknowledgement を入れる。Windowsに比べるとmacOSはまだ遅れているが、display listing だけの段階からはかなり抜けて、実際の映像送信の形が見えてきた。

この時点の進捗見積もり: 96%。

## 2026-05-18 JST - macOS Manual UDP Video Send Probe

今日は macOS host の映像経路をもう一段先に進めた。前回は `ScreenCaptureKit -> VideoToolbox -> GLYF/GLYT packetizer` までだったが、今回は `MacUdpDatagramSender` を追加し、生成した Video-channel datagram を実際にUDP送信できるようにした。SwiftUI には target host / port の入力欄と `UDP Send Probe` ボタンを追加した。

このprobeはまだ正式な approved-client session runtime ではないが、macOSで画面を掴み、H.264へ圧縮し、GlyphRay形式に分割し、手動ターゲットへ送るところまで確認できる。次の焦点は、手動ターゲットをpairing/control runtimeから得た approved Android client に置き換え、continuous video send loop と reconnect/backpressure を持たせること。

この時点の進捗見積もり: 95%。

## 2026-05-15 JST - macOS Video Transport Packetizer

macOS host を急ぎ足で前に出すため、今日は capture / encode の次の段、GlyphRay の Video channel packetizer を Swift 側に実装した。`MacVideoTransportPacketizer` は `MacEncodedFrame` を Rust 側と同じ encoded access unit に包み、`GLYF` fragment に分割し、さらに `GLYT` Video-channel datagram にする。CRC32 もSwift側で持たせたので、macOS host からAndroid clientへ送る直前の形まで作れる。

SwiftUI には `Live Transport Probe` を追加した。ScreenCaptureKit で画面を掴み、VideoToolbox でH.264にし、packetizerでdatagram化して、captured / encoded / datagram count / transport bytes を表示する。まだ UDP 送信と pairing/control runtime はこれからだが、macOS host の映像経路は「掴む、圧縮する、GlyphRay形式に詰める」まで一本につながった。

この時点の進捗見積もり: 94%。

## 2026-05-15 JST - macOS SwiftPM CI

Windows では Swift toolchain を入れられても `SwiftUI` / `ScreenCaptureKit` / `VideoToolbox` を使う macOS host の実ビルド検証には向かないので、GitHub Actions に `macOS host SwiftPM build` job を追加した。`macos-14` runner で `swift --version` と `swift build` を `hosts/macos-host` 配下で実行する。

これで Windows 開発中でも、push / pull request ごとに macOS host のコンパイル崩れをGitHub側で拾える。ローカルWindowsはRust/Android、macOS API部分はActionsのmacOS runner、という役割分担がかなり自然になった。

この時点の進捗見積もり: 94%。

## 2026-05-15 JST - Signed Trusted Reconnect And macOS Encode Probe

今日は Windows 版の trusted reconnect をもう一段固めた。前回は Android Keystore public key の fingerprint 一致までだったが、今回は `AuthChallenge` / `AuthResponse` を実際の接続経路に入れた。Windows host は trusted record に fingerprint と DER public key を保存し、returning device には challenge を返す。Android は Keystore の ECDSA private key で challenge payload に署名し、Windows は P-256 public key で検証してから `PairingResult accepted=true` を返す。

macOS 側も、単なる frame count から一歩進めた。`VideoToolboxEncoder` に実 `CMSampleBuffer` を渡す `encode(sampleBuffer:)` と output callback を追加し、SwiftUI から `Live Encode Probe` を押すと ScreenCaptureKit の frame を低遅延 H.264 encoder に流し、encoded frame count / bytes を確認できる。まだ transport 送信は未接続だが、macOS の capture-to-encode 経路が見えてきた。

この時点の進捗見積もり: 94%。

## 2026-05-15 JST - Windows Trusted Identity And macOS Live Capture Start

今日は Windows 版を完成形に近づけるため、trusted-device を IP 由来の仮IDから Android Keystore public key fingerprint ベースへ寄せた。Android の `PairingRequest` に Keystore 公開鍵 bytes を載せ、Windows host は SHA-256 fingerprint を保存する。次回以降は同じ fingerprint が trusted-device list にある場合だけ auto-approve するので、単なる同一IPよりはかなり製品らしい再接続に近づいた。まだ最終形には challenge/response proof が必要だが、危ない無条件自動承認ではない。

macOS 側にも着手した。`MacLiveCaptureController` を追加し、ScreenCaptureKit の `SCStream` を短時間起動して frame count を取る live capture probe を SwiftUI から走らせられるようにした。まだ VideoToolbox への sample buffer 入力と shared transport 送信はこれからだが、display listing だけだった macOS host が実際に frame を掴みに行く段階へ進んだ。

この時点の進捗見積もり: 93%。

## 2026-05-15 JST - Trusted Device Management

今日は permission dialog の次に必要な trusted-device 管理を進めた。承認済み peer は `LOCALAPPDATA/GlyphRay/host-settings.conf` に trusted-device record として保存され、`trust list`、`trust forget <id>`、`trust clear` で確認・削除できる。record には id、label、last peer、approval timestamp、pen/touch/keyboard/mouse/gamepad の permission flags を持たせた。

ここで自動承認までは進めていない。現時点の trusted id はまだ host 側の管理記録であり、Android の長期 device identity / public key validation と結びついていないため、勝手に approval を bypass させるのは危ない。まず「信頼済みデバイスを見える・消せる」状態を作り、次に暗号ID検証と per-device permission UI につなげる。

この時点の進捗見積もり: 92%。

## 2026-05-15 JST - Native Pairing Permission Dialog

今日は Windows host の pairing approval を console だけの世界から一歩外に出した。`GLYPHRAY_ENABLE_PERMISSION_DIALOG=1` を付けて backend を起動すると、incoming pairing request に対して Win32 の yes/no permission dialog を出せる。dialog は helper thread で開き、backend polling は止めず、結果は console approval と同じ command queue に戻して `approve` / `reject` と同じ経路で `PairingResult` を返す。

stale dialog result にも少しだけ気を配った。例えば console で先に approve/reject したあとに dialog が返ってきても、その peer が pending でなければ結果を無視する。まだ tray 常駐 UI や trusted-device list ではないけれど、接続許可をキーボードコマンドではなく GUI で選べるようになり、ホストアプリらしさが一段増した。

この時点の進捗見積もり: 91%。

## 2026-05-15 JST - Named Encoder Presets

今日は Windows host の stream control を、default override ひとつだけの保存から、名前付き preset の保存・適用・削除へ進めた。`encoder preset save studio-120` で現在の host override、または approved client から届いた `EncoderConfig` を名前付きで保存でき、`encoder preset apply studio-120` で active host override として即座に適用できる。`encoder status` には保存済み default override だけでなく、preset 一覧も出るようにした。

保存形式は既存の `LOCALAPPDATA/GlyphRay/host-settings.conf` を拡張し、`encoder_preset.<index>.*` の key=value として後方互換に寄せた。まだ tray UI ではないけれど、1080p60、1440p120、高bitrate、低bitrate のような検証パターンを console で素早く切り替えられるようになった。unit test では preset の round trip、case-insensitive update、delete、CLI parse を追加した。

この時点の進捗見積もり: 90%。

## 2026-05-15 JST - Persistent Host Encoder Presets

今日は Windows host 側の stream control を、console-only の一時設定から保存できる設定へ進めた。`encoder override <width>x<height> <fps> <kbps>` で作った host override、または approved client から届いた最新 `EncoderConfig` を `encoder save` で保存できる。backend startup 時には保存済み override を読み込み、`encoder clear` は active override と saved override の両方を消す。

保存形式は `LOCALAPPDATA/GlyphRay/host-settings.conf` の小さな key=value file にした。まだ named preset UI ではないけれど、実機で何度も 120fps / 高bitrate / 指定display の検証をするとき、毎回 console に同じ設定を打つ必要がなくなる。unit test では encode/decode、store persist、clear を検証した。

この時点の進捗見積もり: 89%。

## 2026-05-15 JST - Startup And Saved Session Preferences

今日は beta に近づけるための「毎回やり直さなくていい」部分を進めた。Android は video / input preferences を SharedPreferences に保存するようにし、resolution、codec、color space、FPS、bitrate、touch mode、Bluetooth keyboard / mouse / gamepad toggle、fullscreen preference が app restart 後も残るようになった。接続テストのたびに同じ設定を入れ直す小さな摩耗を減らす狙い。

Windows host には user-logon startup の管理を追加した。`glyphray-windows-host startup status|enable|disable` で HKCU Run key を読み書きし、live host console からも `startup status` / `startup enable` / `startup disable` を実行できる。pre-login 接続はまだ約束しないまま、user logon 後の最短起動を実装で一歩進めた。

あわせて `docs/WINDOWS_STARTUP_AND_SERVICE.md` を追加し、今できる user-logon startup と、今後必要な service broker + per-user agent 分離、lock screen / secure desktop の制約を明文化した。検証として Rust workspace tests、Android unit tests、Android debug build を通した。

この時点の進捗見積もり: 88%。

## 2026-05-13 JST - Persistence, Touch Modes, Display Mapping, DPAPI

今日は「100%に限りなく近づける」方向で、見た目よりも実機運用で効く土台を進めた。Android の manual host entry は、その場限りではなく SharedPreferences に保存され、次回起動時に host list へ復元されるようになった。Tailscale IP や MagicDNS name でつなぐ検証が、毎回打ち直しではなくなる。

入力まわりは、finger touch を単に native touch として流すだけでなく、touch mode に応じて direct / trackpad / gesture assist の挙動を分けた。direct は `TouchInputBatch` のまま送り、trackpad は one-finger movement を synthetic mouse movement に変換し、gesture assist は two-finger movement を wheel delta として送る。これで Android tablet を机上の remote surface として使う時の操作感を少し現実に寄せられた。

Windows host は pen / mouse / touch injection の runtime mapper を固定 1920x1080 から、可能な場合は選択 display geometry ベースに変えた。Android video settings には host から届いた display list を選ぶ UI を追加し、選択した display id を stylus / touch / mouse packet へ乗せるようにしたので、multi-monitor 検証の入口がつながった。さらに `PlatformSecretStore` を DPAPI 保護の per-user file store に置き換え、ペアリング済み端末の長期 secret を開発用メモリ保存から一段引き上げた。検証として Windows host tests、Android unit tests、Android debug build を通した。

この時点の進捗見積もり: 87%。

## 2026-05-13 JST - Fullscreen And Host Encoder Control

今日は前回の UI/UX push の続きとして、実際の利用時に効く操作感と host control を進めた。Android session の Full ボタンは、bottom navigation を隠すだけではなく、Android の status bar / navigation bar まで隠す immersive mode に入るようにした。active session 中は `FLAG_KEEP_SCREEN_ON` も立てるので、描画中や検証中に画面が寝てしまう事故を減らせる。

Windows host 側では、approved client から届いた `EncoderConfig` を video pump に反映する道をつないだ。`GLYPHRAY_ENABLE_VIDEO_STREAM=1` で動く pump は、client config 更新時に再起動し、FPS interval、bitrate、codec、color space、keyframe interval を effective encoder settings に入れる。resolution はまだ scaler が無いため capture-native に戻すが、その制限は console に明示する。

さらに host console に `encoder status`、`encoder override <width>x<height> <fps> <kbps>`、`encoder clear` を追加した。これで client request だけでなく host operator 側からも stream settings を切り替えられる。検証として Windows host tests、Android unit tests、Android debug build を通した。

この時点の進捗見積もり: 84%。

## 2026-05-13 JST - Android UI/UX Cockpit Pass

今日は UI/UX に集中した。Android client は、単に画面がある状態から、接続前・接続中・設定変更の判断がしやすい作業画面へ寄せた。Host list には discovery 状態、manual endpoint、host capability pill を追加し、Connect 画面には target / requested session / readiness checklist を置いた。Session 画面は remote surface の上に host、RTT、stream、input 状態を並べ、下には video / input counters を整理したので、LAN smoke test 中に何が動いているか追いやすくなった。

Pen settings は pressure curve と mapping mode を選択状態つきの chip にし、pressure preview を入れた。Video settings は resolution、refresh rate、bitrate、codec、color space、client controls をそれぞれ操作しやすい group に分けた。Security と diagnostics も status band と information panel へ揃え、全画面を scrollable frame にしたので、電話サイズでも詰まりにくい。

公開用 GitHub Pages site も progress を 83% に更新し、色調を graphite / teal / amber / rose の組み合わせへ調整して、単調な茶色寄りに見えないようにした。Android debug build は通過済み。

この時点の進捗見積もり: 83%。

## 2026-05-12 JST - Video Channel And Ink Smoothing Push

今日は video / pen / Android transport を一気に前へ押した。Rust 側では capture -> encode -> packetize の既存部品に `VideoPacketPipeline` を足し、Windows backend runtime から approved peer に `VideoFrame` packet を Video channel で queue できるようにした。`GLYPHRAY_ENABLE_VIDEO_STREAM=1` で video pump が動き、H.264 access unit envelope を `GLYF` fragment に分割して outbound QoS queue に載せる。まだ default encoder は placeholder なので、実デスクトップ映像には concrete H.264 backend が必要。

Windows Ink 側は `StylusInputBridge` に pressure smoothing と pen axis normalization を入れた。Android の historical samples が一気に届いても、Win32 synthetic pen injection に渡す pressure が急に跳ねにくくなる。tilt は -90..90 に clamp、orientation は 0..360 に正規化する。unit test では smoothing と axis normalization を検証した。

Android 側は transport codec に Video channel `VideoFrame` encode/decode と QoS send queue を追加し、control receiver が Video packet を `RemoteVideoStreamController` に流せるようにした。Remote display surface が decoder を作ったら session controller に接続し、Video fragment -> access unit -> MediaCodec の入口までつながる。これで video/control/input が同じ transport shape を共有し始めた。

検証として Rust workspace tests、Android unit tests、Android debug build を通した。

この時点の進捗見積もり: 82%。

## 2026-05-12 JST - macOS Host Readiness Push

今日は macOS host の進み具合を確認した。現状は SwiftUI shell、ScreenCaptureKit display listing、VideoToolbox encoder setup、CGEvent mouse posting、audio permission plumbing までは入っていたが、Windows backend ほど runtime 化は進んでいない。なので、今回は macOS の実機検証を進めやすくする readiness 層を追加した。

実装として、Screen Recording / Accessibility / Input Monitoring / audio の permission 状態を UI に出し、Screen Recording と Accessibility は request button から prompt できるようにした。ScreenCaptureKit の display listing は geometry 付き descriptor に変え、VideoToolbox は H.264 low-latency encoder session の smoke test を UI から走らせられる。CGEvent は mouse move / click / keyboard foundation まで広げた。

さらに macOS Keychain 用の `KeychainSecretStore` を追加し、UI から save / load / delete の smoke test を走らせられるようにした。まだ device identity や trusted host list に完全接続していないが、長期secretを平文保存しないための platform boundary はできた。次は `SCStream` から frame を受けて VideoToolbox encoder に入れ、shared transport へ流す macOS live stream path に進む。

この時点の進捗見積もり: 80%。

## 2026-05-12 JST - Backend Health Visibility

今日は、入れてもらった backend hardening を確認し、その上に「見える化」を足した。pending session cap、IPごとの pending attempt rate limit、late input drop、channel-aware QoS outbound queue は良い方向にまとまっていたので、そこを壊さずに `BackendHealthSnapshot` と console の `status` command を追加した。

`status` では session 数、pending 数、input/control/audio/video 別 queue depth、queue high watermark、outbound drop、late input drop、pending rate limit、backpressure event を見られる。まだ専用 send worker / event loop ではないが、LAN smoke test 中に「詰まっているのか、落としているのか、rate limit が効いているのか」をホスト側で確認できるようになった。

検証として Rust workspace tests を通した。Android 側のコードは今回は触っていないが、後続で Android debug build も再確認する。

この時点の進捗見積もり: 79%。

## 2026-05-12 JST - Host Router Hardening

今日は host backend の守りと低遅延性を補強した。未知 peer からの packet を無制限に `SessionRegistry` へ積むと、送信元 port を変えた spam で pending session が増え続けるため、pending session を最大 50 件に制限し、超過時は最古の pending peer を破棄するようにした。approved/rejected session は保持し、未承認の掃除だけに絞っている。

UDP の順序逆転対策として、approved session ごとに最新 input sequence と timestamp を記録し、それより古い stylus/touch/mouse/keyboard/gamepad packet は injection 前に drop するようにした。絵を描く用途では「遅れて届いた古い座標で一瞬戻る」ほうが破壊的なので、遅延 packet の救済より現在位置の安定を優先した。

control response 送信は `poll_control` の hot loop から bounded queue に逃がし、`try_send_to` で nonblocking flush する短期対策を入れた。専用 send worker や mio/tokio event loop はまだ次段階だが、受信 loop が送信詰まりで長く止まるリスクは下げた。あわせて discovery host id は独自 hash から `crc32fast` ベースの stable id に置き換えた。

追加で、単一IPが送信元portを変え続けて正規pending peerを追い出す starvation を抑えるため、IPごとの新規pending attempt rate limit を入れた。outbound queue も単一 `VecDeque` から channel 別 queue + QoS schedule に変え、control/input が video backlog に埋もれない形へ寄せた。pending eviction は今は上限50なので O(N) scan のままで十分だが、Relay server など大規模用途へ転用する場合は heap / indexed map へ移す。

検証として Rust workspace tests と Android unit tests を通した。Gradle wrapper は Java 24 対応のため 8.14.3 に更新済み。

この時点の進捗見積もり: 78%。

## 2026-05-12 JST - Touch, Mouse, Gamepad, Tailscale, Packaging

今日は入力まわりを Parsec 的な広さに寄せた。Android finger touch を stylus 代替で流すだけでは Windows touch 対応デバイスと同じ挙動にはならないため、protocol に `TouchInputBatch` を追加した。Android は finger touch を native touch packet として送り、Windows host は `GLYPHRAY_ENABLE_TOUCH_INJECTION=1` のとき `PT_TOUCH` として注入できる smoke-test path を持つようになった。まだ temporary 1920x1080 mapping なので、完全に「Windows touch device と同じ」と言うには calibration / monitor negotiation / multi-touch validation が残っている。

Bluetooth mouse は `MouseInput` として分離し、Windows host は `GLYPHRAY_ENABLE_MOUSE_INJECTION=1` で cursor / buttons / wheel を注入できるようにした。Game controller は Android の gamepad buttons / axes を `GamepadInput` で送れるようにし、Windows host は decode まで対応した。実際に Windows に Xbox controller として見せるには ViGEm などの virtual gamepad backend が必要なので、roadmap に入れた。

Tailscale は broadcast discovery が届かない前提で、Android host list に manual host entry を追加した。Tailscale IP や MagicDNS name を入れれば、同じ UDP control/input ports で接続できる。installer / release については Windows WiX MSI script、macOS pkgbuild script、Play Store internal testing に向けた release document を追加した。

検証として Rust workspace tests と Android debug build を通した。

この時点の進捗見積もり: 77%。

## 2026-05-12 JST - Client And Host Control Audit

今日は、解像度、refresh rate、bitrate、color space、codec、touch、Bluetooth keyboard、fullscreen、Win/PrintScreen 補助キー、host startup / pre-login 接続について棚卸しした。結論として、video protocol は一部あったが色空間と client 設定送信が足りず、touch/keyboard/fullscreen/special keys はまだ入口だった。pre-login 接続は Windows の interactive desktop / secure desktop 制約があるため、サービス化しても慎重な設計が必要。

実装として、protocol の `EncoderConfig` に `ColorSpace` を追加し、Android の Video Settings から resolution / refresh / bitrate / color space / codec を選んで host へ送れるようにした。host backend は approved client の `EncoderConfig` を decode して session に保持する。さらに Android remote surface は Bluetooth keyboard の `KeyEvent` を拾い、common key を Windows virtual key に変換して `KeyboardInput` として送れるようにした。Win / PrintScreen の補助キー overlay も追加し、host 側は opt-in の `SendInput` keyboard injection wrapper まで進めた。

fullscreen は Android session の bottom navigation を隠す focus mode として実装した。system bar まで隠す本当の immersive fullscreen、touch gesture translation、host-side encoder override UI、startup-at-login / service-agent 構成は `docs/FEATURE_MATRIX.md` と roadmap に明記した。

検証として Rust workspace tests と Android debug build を通した。Android unit test task はこのローカル環境の JDK 24 だと AGP の task 生成で落ちるため、CI と同じ JDK 17 で回す前提にしている。

この時点の進捗見積もり: 75%。

## 2026-05-12 JST - DisplayInfo Handshake

今日は pairing の後に host display geometry を返す `DisplayInfo` handshake を追加した。Windows backend は accepted pairing の直後に monitor enumeration の結果を `DisplayInfo` として control channel に流す。manual approval でも development auto-approval でも同じように display info が queued される。

Android 側には `DisplayInfo` decode を追加し、`RemoteDisplayDescriptor` として保持するようにした。Connect 画面には host display count と primary display label が出る。これで、次に selected monitor、DPI、rotation、calibration を本物の host geometry に合わせる準備が整った。

検証として Rust workspace tests と Android debug build を通した。

この時点の進捗見積もり: 73%。

## 2026-05-12 JST - Pairing Response Loop

今日は Android と Windows host の control channel を片道から往復にした。Windows backend は `PairingRequest` を受けたあと、console の `approve <peer>` / `reject <peer>` で `PairingResult` を返せるようになった。development auto-approval では pairing request 自体にも accepted result を返すので、実機 smoke test の手順が短くなる。

Android 側も `GLYT` datagram と `GLYR` frame の response decode を追加し、`PairingResult` と `LatencyPong` を `SessionControlState` に反映するようにした。Connect 画面には pairing accepted/rejected、trusted device id、latency pong の簡易表示が出る。これで host discovery、control request、host approval、control response までが 1 本につながった。

検証として Rust workspace tests と Android debug build を通した。Android unit test は引き続き JDK 17 の CI 側で見る前提。

この時点の進捗見積もり: 72%。

## 2026-05-12 JST - Pages Permission Fix

GitHub Pages workflow で `enablement: true` を使って Pages site の自動作成を試したが、この repository では `GITHUB_TOKEN` が Pages site creation API にアクセスできず、`Resource not accessible by integration` になった。workflow から site を作る方式は諦め、GitHub settings で Pages を一度だけ手動有効化してから deploy する前提に戻した。

同時に GitHub Actions の Node.js 20 deprecation warning に備えて、workflow env に `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` を追加した。これで Pages workflow は「手動有効化済みの Pages に artifact を deploy する」役割へ絞られた。

この時点の進捗見積もり: 71%。

## 2026-05-12 JST - Pages Enablement Fix

GitHub Pages workflow の初回実行で、repository の Pages site がまだ存在しないため `actions/configure-pages` が `Get Pages site failed` で止まった。`configure-pages@v5` に `enablement: true` を渡すようにして、workflow 側から Pages の初回有効化を試せる構成へ修正した。

これでも organization policy などで自動有効化が拒否される場合は、GitHub repository settings から Pages を手動で有効化し、source を GitHub Actions にする必要がある。その注意も README と website README に追記した。

この時点の進捗見積もり: 71%。

## 2026-05-12 JST - GitHub Pages Download Site

今日は GlyphRay の公開入口として、frontend-only の GitHub Pages download site を追加した。`website/` に静的な `index.html` / `styles.css` / `app.js` を置き、ビルドなしでそのまま Pages artifact として配信できる構成にした。

サイトは単なるリンク集ではなく、Android client、Windows host、macOS host の状態を分けて見せる download card と、ローカル setup command generator を持つ。ブラウザだけでは S Pen の高頻度入力や UDP transport、Windows Ink injection はできないので、そこは正直に native app が必要だと明記した。hero にはオリジナルの生成 PNG アートを入れて、GlyphRay の「ペンでデスクトップを触る」雰囲気が最初の画面で伝わるようにした。

この時点の進捗見積もり: 71%。

## 2026-05-12 JST - Android Control Channel

今日は Android 側に session control の送信経路を追加した。Host list で見つけた host を選び、Connect 画面の Start session を押すと、`GLYT` control datagram の中に `GLYR` の `PairingRequest` と `LatencyPing` を包んで Windows host へ送る。

Kotlin 側には Rust の `bincode` enum layout に合わせた最小限の `ProtocolFrameCodec` も入れた。これで stylus UDP だけでなく、接続開始時の control path も Android から実際に動かせる。あわせて protocol frame の JVM unit test を追加し、GitHub Actions の Android job でも `testDebugUnitTest` を走らせるようにした。ローカル JDK 24 では Android Gradle Plugin の unit test task 生成が落ちるため、README には JDK 17 を使う注意も追記した。

この時点の進捗見積もり: 70%。

## 2026-05-11 JST - Day 1, Milestone 1 Foundation

今日は GlyphRay が、ただの構想から実際のリポジトリになった日。モノレポ構造、Rust の共有 crate、Android アプリの入口、Windows host、macOS shell、protocol 定義、最初のテスト、そして product/security/architecture docs を一気に敷いた。

いちばん大事な背骨は stylus path。Android 側では raw `MotionEvent` を見られるようになり、Windows 側には `CreateSyntheticPointerDevice` / `InjectSyntheticPointerInput` を使う native synthetic pen injection wrapper が入った。まだ端から端までつながってはいないけれど、GlyphRay が何者になるかはコードの形として見え始めた。

この時点の進捗見積もり: 20%。

## 2026-05-11 JST - Day 1, Milestone 2 First Pass

今日は Milestone 2 に踏み込んだ。Android には low-latency `SurfaceView` と `MediaCodec` H.264 decoder wrapper を追加し、remote session 画面には latency overlay を載せた。Windows host には monitor enumeration、low-latency H.264 encoder abstraction、そして後続の hardware/software backend を差し込むための境界を追加した。

transport 側では UDP datagram packet format を作った。channel、message kind、sequence、timestamp、payload length、checksum を持つ小さな層で、input/control packet にはもう使える形になっている。さらに video frame の chunking/reassembly utility も入れたので、大きい H.264 frame を UDP packet に分ける準備まで進んだ。

README も整えて、開発進捗率を毎回更新する場所を作った。正直な現在地は、Milestone 1 は完了、Milestone 2 は映像と transport の配管が組み上がり始めたところ。次の山は Windows capture と、実際の H.264 frame を LAN で Android decoder に流し込むところ。

この時点の進捗見積もり: 31%。

## 2026-05-11 JST - Day 1, Milestone 2 Video Path Deepening

今日は映像経路をもう一段つないだ。transport には encoded video access unit を `TransportPacket` 群へ変換し、受信側で再構成する `VideoPacketizer` / `VideoReassembler` を追加した。前回の fragment utility が部品だとすると、今回は「H.264 frame を実際に運ぶための箱」まで作った感じ。

Windows host には capture -> encode -> packetize -> transport send の `VideoStreamPipeline` を追加した。capture は本命の Windows Graphics Capture / Desktop Duplication ではまだないけれど、早期検証用に GDI fallback を入れて、選択 monitor から BGRA frame を取れる形にした。さらに `glyphray-capture-diagnostics` も追加したので、実機 Windows では monitor 列挙と 1 frame capture の smoke test ができる。

Android 側には Rust と同じ `GLYF` fragment と encoded access unit envelope を読む `VideoFragmentReassembler`、そして完成した H.264 access unit を `RemoteVideoDecoder` に渡す `RemoteVideoStreamController` を追加した。これで host と Android の video path が、まだ LAN loop は無いものの、同じ protocol shape を見ている。

この時点の進捗見積もり: 34%。

## 2026-05-11 JST - Day 1, Milestone 3-5 Acceleration

今日は「Milestone 5 まで一気に」というリクエストに合わせて、完成品として嘘をつかない範囲で、先の milestone に必要な骨組みをかなり広げた。Android には compact stylus packet encoder と calibration UI surface、Keystore の device identity key foundation を追加した。Rust protocol 側にも同じ `GLYS` stylus wire format を入れ、Windows host には remote stylus batch を native pen injector に渡す bridge を入れた。

Milestone 4 側では ChaCha20-Poly1305 の session cipher、replay guard、secure datagram codec、reconnect backoff、adaptive bitrate controller、host diagnostics CLI、packaging foundation を追加した。これで security と reliability の話が、文書だけでなくコード上の差し込み口として見えるようになった。

Milestone 5 側では audio packetization crate、relay candidate selection、macOS VideoToolbox encoder foundation、CGEvent input foundation、audio permission plumbing、beta checklist を追加した。まだ実機での live capture/encode/relay/audio playback までは届いていない。でも、Windows/Android/macOS/transport/security/audio/relay の主要な境界はそろった。

この時点の進捗見積もり: 58%。

## 2026-05-11 JST - Day 1, Backend Runtime Push

今日は backend の芯を厚くした。transport に LAN discovery advertisement (`GLYD`) と server-side UDP socket を追加し、Windows host には `HostBackendRuntime`、`SessionRegistry`、`HostPacketRouter` を入れた。これで host は「LAN 上で見つかる」「peer を session として覚える」「未承認 peer の input を止める」「pairing request を拾う」「approved peer の stylus packet を native pen bridge へ送る」という backend らしい振る舞いを持ち始めた。

`glyphray-windows-host serve` も追加した。まだ console loop で、approval UI や continuous video loop はこれからだけれど、今までの部品が backend runtime としてまとまり始めたのは大きい。次は host UI から peer approve/reject を動かし、video pipeline を runtime loop に接続する。

この時点の進捗見積もり: 62%。

## 2026-05-11 JST - Day 1, CI Repair Pass

今日は GitHub Actions で落ちていた Rust tests と Android build の修正に集中した。Rust 側は `hmac::Mac` と `chacha20poly1305::KeyInit` がどちらも `new_from_slice` を持つため、HMAC 初期化を明示的に `Mac` 側へ解決するようにした。

Android 側は Compose の `Column` を CI で確実に解決できるよう `foundation-layout` を明示依存に追加し、stylus diagnostics の `pointerInteropFilter` は `onTouchEvent` named argument で渡すようにした。小さい修正だけれど、CI が赤いままだと開発速度が落ちるので、ここは地味に大事な前進。

この時点の進捗見積もり: 63%。

## 2026-05-11 JST - Day 1, Android LAN Path

今日は Android 側が LAN 上の GlyphRay host を自分で見つけるところまで進めた。Rust host が投げる `GLYD` advertisement を Kotlin で decode する `HostDiscovery` を追加し、Host list 画面は固定ダミーではなく discovery state を表示する構造に変わった。

さらに Android から `GLYS` stylus payload を `GLYT` UDP datagram に包む `TransportPacketCodec` と `StylusUdpSender` を追加した。Windows backend 側には、approval UI ができるまでの実機 smoke test 用として `GLYPHRAY_DEV_AUTO_APPROVE` の明示的な開発モードを入れた。まだ production pairing ではないけれど、Android のペン入力を LAN packet として host backend に届ける足場ができた。

この時点の進捗見積もり: 65%。

## 2026-05-11 JST - Day 1, README Visual Pass

今日は README を、ただの説明文から「リポジトリの地図」に近づけた。`README.md` には進捗 pie chart、milestone progress bar、monorepo map、runtime flow、system shape を追加し、初見でも Android / Windows host / Rust crates / docs がどう関係するか分かるようにした。

さらに日本語版の `README.ja.md` を追加した。GlyphRay は扱っている領域が Android、Windows native、Rust protocol、security、低遅延 video と広いので、日本語で全体像を掴める入口があるだけで開発の迷子率がかなり下がるはず。

この時点の進捗見積もり: 66%。

## 2026-05-11 JST - Day 1, Live Stylus Sender

今日は Android の remote session 画面を、ただの表示面から「ペン入力を host に送る面」に近づけた。`StylusLanBridgeController` を追加して、選択済み host へ `GLYS` stylus packet を `GLYT` UDP datagram として送る background worker を作った。

`RemoteDisplayView` も入力 callback を受け取れるようにし、内部の `SurfaceView` で stylus `MotionEvent` を拾えるようにした。これで Host discovery、Host selection、Session screen、Stylus capture、UDP sender が Android 側ではかなり一本の線になった。まだ production pairing と Windows native injection への完全な end-to-end 検証は残るけれど、Milestone 3 の背骨は太くなってきた。

この時点の進捗見積もり: 67%。

## 2026-05-11 JST - Day 1, Opt-In Pen Bridge

Android から stylus packet を送れるようになったので、今日は Windows backend の受け側を一段進めた。`Box<dyn PenInjector>` を backend runtime に差し込めるようにして、`GLYPHRAY_ENABLE_PEN_INJECTION=1` のときだけ native Win32 synthetic pen injector bridge を使う道を作った。

これはまだ production の接続ではない。mapping は一時的な 1920x1080 stretch で、approval UI も本番 handshake もこれから。でも、`GLYPHRAY_DEV_AUTO_APPROVE=1` と組み合わせると、Android remote surface から Windows host の pen injection までを smoke test できる形に近づいた。

この時点の進捗見積もり: 68%。

## 2026-05-11 JST - Day 1, Android Build Green

Android debug build が `gradlew.bat :apps:android-client:assembleDebug` で成功した。残っていた Compose の `Divider` deprecation warning は `HorizontalDivider` に置き換えた。Rust と Android の両方で実コンパイルが通ったので、次は実機 LAN smoke test に進める状態になった。

この時点の進捗見積もり: 69%。

## 2026-05-12 JST - CI Gradle Wrapper Permission

GitHub Actions の Android build で `./gradlew: Permission denied` が出た。Windows では実行権限ビットが見えにくいので、Linux runner 上で `chmod +x ./gradlew` を CI step として明示的に入れた。これで wrapper を使う方針を保ったまま、Actions 側でも assembleDebug に進める。

この時点の進捗見積もり: 69%。

## 2026-05-11 JST - Day 1, Compose Scope Compile Fix

Gradle 8.11.1 に固定した後、Android compile は `Column` unresolved で止まった。原因は Compose の `Column` 関数を `ScreenFrame` の receiver 型として使っていたこと。型として正しいのは `ColumnScope` なので、`content: @Composable ColumnScope.() -> Unit` に修正した。

この時点の進捗見積もり: 68%。

## 2026-05-11 JST - Day 1, Gradle Wrapper Pin

Android build の `debugRuntimeClasspathCopy` error は Gradle 9.0 milestone と Android Gradle Plugin 8.7.3 の組み合わせで出ていた。wrapper が Gradle 9.0 milestone を指していたので、CI と同じ Gradle 8.11.1 に固定し、README と CI も `gradle` ではなく `./gradlew` / `gradlew.bat` を使う形に揃えた。

この時点の進捗見積もり: 68%。

## 2026-05-11 JST - Day 1, Android Gradle Configuration Fix

Android build で `debugRuntimeClasspathCopy` が resolution root と consumable variant の両方として扱われる Gradle configuration error が出た。Android module 側で `*RuntimeClasspathCopy` configuration を明示的に `canBeConsumed=false` にして、依存解決用 configuration としてだけ扱うようにした。

この時点の進捗見積もり: 68%。

## 2026-05-11 JST - Day 1, Rust Test Cleanup

Windows 実コンパイルが通った後に出た残りは、coordinate mapping の期待値ズレと GDI cleanup warning だった。`1600x1000` を `1920x1080` に Fit する場合は上下 letterbox ではなく左右 pillarbox になるので、test の期待値を x=96..1824 / y=0..1080 に直した。

GDI cleanup は `DeleteDC` / `DeleteObject` の戻り値を明示的に捨てるようにして、warning を消した。ここまで来ると、Rust 側はかなり普通の unit test failure の世界に戻ってきた感じがある。

この時点の進捗見積もり: 68%。

## 2026-05-11 JST - Day 1, Windows API Compile Fix

今日は Windows 実コンパイルで見つかった `windows` crate 0.58 の API 配置差分を直した。GDI capture 側は `GetDC` / `ReleaseDC` の import 先、`MONITORINFOF_PRIMARY` の namespace、handle の null pointer 判定、`BitBlt` の `Result` 戻り値に合わせて修正した。

synthetic pen injection 側は、`CreateSyntheticPointerDevice` / `DestroySyntheticPointerDevice` / `POINTER_TYPE_INFO` が `Win32_UI_Controls` 側にあるため feature と import を追加し、pen mask は `WindowsAndMessaging` の `PEN_MASK_*` を使う形に直した。これで少なくとも報告された unresolved import / type mismatch 系は潰した。

この時点の進捗見積もり: 68%。
