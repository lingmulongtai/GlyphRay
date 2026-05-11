# GlyphRay Development Diary

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
