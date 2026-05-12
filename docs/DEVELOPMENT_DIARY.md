# GlyphRay Development Diary

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
